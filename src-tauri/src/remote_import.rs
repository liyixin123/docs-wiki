//! Importing a GitHub repository (or a subdirectory of one) as a new
//! documentation source. Lists markdown files via the Git Trees API (one
//! request lists the whole repo, avoiding per-directory rate-limit
//! multiplication), derives a nav order the same way local-folder import
//! does (a manifest file at the root of the path if present, otherwise
//! filesystem-shape fallback), and downloads every file's raw content via
//! `raw.githubusercontent.com`.
//!
//! Unlike local-folder import, a freshly imported remote source is always
//! single-language: there's no upstream "translated" copy to mirror, only
//! the primary-language content itself — a second language shows up once
//! the (future) translate pipeline produces one.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::Deserialize;

use crate::hashing::sha256_hex;
use crate::local_import::{slugify, unique_source_id};
use crate::nav::{natural_cmp, parse_link_manifest, prettify_id, NavCategory, NavItem, NavTree};
use crate::sources::write_manifest;
use crate::state::{
    now_iso, AppStateData, CheckStatus, DocMeta, LogEntry, LogKind, RemoteSpec, Source, SourceKind,
    TranslationStatus,
};

const USER_AGENT: &str = "docswiki-app";
const MANIFEST_FILE_NAMES: &[&str] = &["summary.md", "_sidebar.md", "readme.md"];

#[derive(Deserialize)]
struct TreeResponse {
    tree: Vec<TreeEntry>,
    #[serde(default)]
    truncated: bool,
}

#[derive(Deserialize)]
struct TreeEntry {
    path: String,
    #[serde(rename = "type")]
    kind: String,
}

/// Import `spec` (owner/repo/branch/path) as a new source. Does the network
/// fetching itself; does not persist `data` to disk — the caller does that
/// once the state lock is released.
///
/// `translate_to` optionally registers a second, not-yet-translated language
/// (e.g. `zh`): every doc starts out `NeverTranslated`, which is what lights
/// up the translation banners/buttons in the UI. Without it the source stays
/// single-language and translation is disabled for it, as before.
pub async fn import_remote_source(
    dir: &Path,
    data: &mut AppStateData,
    spec: RemoteSpec,
    display_name: Option<String>,
    lang: String,
    translate_to: Option<String>,
) -> Result<Source> {
    validate_lang_code(&lang, "源语言")?;
    let target_lang = translate_to
        .map(|t| t.trim().to_lowercase())
        .filter(|t| !t.is_empty());
    if let Some(target) = &target_lang {
        validate_lang_code(target, "翻译目标语言")?;
        if target == &lang {
            bail!("翻译目标语言不能与源语言相同（都是 '{lang}'）");
        }
    }
    let languages = match &target_lang {
        Some(target) => vec![lang.clone(), target.clone()],
        None => vec![lang.clone()],
    };
    // With a translation target registered, no target-language files exist
    // yet — same situation as a bilingual local import missing its secondary
    // folder, so reuse that status.
    let initial_status = if target_lang.is_some() {
        TranslationStatus::NeverTranslated
    } else {
        TranslationStatus::NotApplicable
    };

    let cfg = crate::config::load_config(dir).unwrap_or_default();
    let client = github_client(crate::config::github_token_for(&cfg).as_deref());
    let all_paths = fetch_tree_paths(&client, &spec).await?;

    let prefix = normalized_prefix(&spec.path);
    let rel_paths: Vec<String> =
        all_paths.iter().filter(|p| p.starts_with(&prefix)).map(|p| p[prefix.len()..].to_string()).collect();

    let md_ids: Vec<String> =
        rel_paths.iter().filter(|p| p.ends_with(".md")).map(|p| p.trim_end_matches(".md").to_string()).collect();
    if md_ids.is_empty() {
        bail!("'{}/{}' 下（{}）没有找到任何 Markdown 文档", spec.owner, spec.repo, display_path(&spec.path));
    }

    let manifest_rel_path = find_manifest_path(&rel_paths);
    let manifest_content = match &manifest_rel_path {
        Some(mp) => fetch_raw(&client, &raw_url_for_rel_path(&spec, mp)).await.ok(),
        None => None,
    };
    let mut categories = manifest_content.as_deref().map(parse_link_manifest).unwrap_or_default();
    if categories.is_empty() {
        categories = build_fallback_categories_from_ids(&md_ids);
    }

    let name = display_name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| format!("{}/{}", spec.owner, spec.repo));
    let source_id = unique_source_id(data, &slugify(&name));

    let manifest_doc_id = manifest_rel_path.as_deref().map(|p| p.trim_end_matches(".md").to_string());
    let dest_root = dir.join("sources").join(&source_id).join("docs").join(&lang);
    std::fs::create_dir_all(&dest_root)?;

    let mut hashes: HashMap<String, String> = HashMap::new();
    let mut doc_contents: Vec<(String, String)> = Vec::new();
    for id in &md_ids {
        if manifest_doc_id.as_deref() == Some(id.as_str()) {
            continue; // the manifest file itself isn't a browsable doc
        }
        let url = raw_url_for_rel_path(&spec, &format!("{id}.md"));
        let content = fetch_raw(&client, &url).await.with_context(|| format!("下载 {id}.md 失败"))?;
        let dest_path = dest_root.join(format!("{id}.md"));
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&dest_path, &content).with_context(|| format!("writing {dest_path:?}"))?;
        hashes.insert(id.clone(), sha256_hex(&content));
        doc_contents.push((id.clone(), content));
    }

    download_referenced_assets(dir, &source_id, &spec, &rel_paths, &doc_contents, &client).await;

    let now = now_iso();
    let mentioned: HashSet<&str> = categories.iter().flat_map(|c| c.items.iter().map(|i| i.doc_id.as_str())).collect();

    let mut doc_metas = Vec::new();
    for cat in &categories {
        for item in &cat.items {
            let Some(hash) = hashes.get(&item.doc_id) else { continue };
            doc_metas.push(make_remote_doc_meta(&source_id, &item.doc_id, item.title.clone(), cat.name.clone(), hash.clone(), initial_status, &now));
        }
    }
    for id in &md_ids {
        if mentioned.contains(id.as_str()) || manifest_doc_id.as_deref() == Some(id.as_str()) {
            continue;
        }
        let Some(hash) = hashes.get(id) else { continue };
        let title = prettify_id(id.rsplit('/').next().unwrap_or(id));
        doc_metas.push(make_remote_doc_meta(&source_id, id, title, "未分类".to_string(), hash.clone(), initial_status, &now));
    }

    if doc_metas.is_empty() {
        bail!("导入失败：清单中提到的文档在仓库里都找不到对应文件");
    }

    write_manifest(dir, &source_id, &NavTree { source_id: source_id.clone(), categories })?;

    let source = Source {
        id: source_id.clone(),
        name: name.clone(),
        kind: SourceKind::RemoteGit,
        languages,
        primary_language: lang,
        remote: Some(spec.clone()),
        local_path: None,
        order_override: Vec::new(),
        created_at: now.clone(),
        updated_at: now.clone(),
    };

    let doc_count = doc_metas.len();
    data.sources.push(source.clone());
    data.docs.extend(doc_metas);
    data.log.push(LogEntry {
        id: format!("import-{source_id}-{now}"),
        ts: now.clone(),
        source_id: source_id.clone(),
        doc_id: None,
        kind: LogKind::Import,
        detail: format!("Imported remote source '{name}' from {}/{} ({doc_count} docs)", spec.owner, spec.repo),
        snapshot: None,
    });

    Ok(source)
}

/// Scan every downloaded doc's markdown for image references and download
/// the in-repo ones (relative to each doc's directory, or `/…` from the
/// repo root) into `sources/<id>/`, preserving their repo-relative paths.
/// Only files present in the fetched tree are fetched; each failure is
/// logged and skipped so one broken image can't fail the whole import.
async fn download_referenced_assets(
    dir: &Path,
    source_id: &str,
    spec: &RemoteSpec,
    rel_paths: &[String],
    doc_contents: &[(String, String)],
    client: &Client,
) {
    let tree: HashSet<String> = rel_paths.iter().cloned().collect();
    for asset in collect_referenced_assets(doc_contents, &tree) {
        let url = raw_url_for_rel_path(spec, &asset);
        match fetch_raw_bytes(client, &url).await {
            Ok(bytes) => {
                let dest = dir.join("sources").join(source_id).join(&asset);
                if let Some(parent) = dest.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Err(err) = std::fs::write(&dest, &bytes) {
                    eprintln!("asset write failed for {asset}: {err}");
                }
            }
            Err(err) => eprintln!("asset download failed for {asset}: {err}"),
        }
    }
}

/// Collect the repo-relative paths of every in-repo image referenced by the
/// given docs' markdown (both `![…](…)` and HTML `<img src="…">`).
/// External URLs and refs missing from `tree` are dropped; `..` segments
/// escaping the repo root are rejected outright.
fn collect_referenced_assets(
    docs: &[(String, String)],
    tree: &HashSet<String>,
) -> std::collections::BTreeSet<String> {
    let mut assets = std::collections::BTreeSet::new();
    for (id, content) in docs {
        let doc_dir = match id.rfind('/') {
            Some(i) => &id[..i + 1],
            None => "",
        };
        for src in image_refs(content) {
            if src.starts_with("http://") || src.starts_with("https://") || src.starts_with("data:") {
                continue;
            }
            if let Some(rel) = resolve_repo_path(doc_dir, &src) {
                if tree.contains(&rel) {
                    assets.insert(rel);
                }
            }
        }
    }
    assets
}

/// Build the reqwest client used for all GitHub traffic (API + raw). When
/// a token is available it rides along as a default Authorization header on
/// every request, lifting the anonymous 60/h rate limit to 5000/h.
pub(crate) fn github_client(token: Option<&str>) -> Client {
    let mut headers = reqwest::header::HeaderMap::new();
    if let Some(t) = token {
        if let Ok(v) = reqwest::header::HeaderValue::from_str(&format!("Bearer {t}")) {
            headers.insert(reqwest::header::AUTHORIZATION, v);
        }
    }
    Client::builder().default_headers(headers).build().unwrap_or_default()
}

/// Extract image srcs from markdown: `![alt](src)` and `<img src="…">`.
/// Scans only at char boundaries (a byte-wise walk would slice mid-char on
/// multibyte text and panic).
fn image_refs(content: &str) -> Vec<String> {
    let mut refs = Vec::new();
    let mut from = 0;
    while let Some(rel) = content[from..].find("![") {
        let bang = from + rel;
        if let Some(close) = content[bang + 2..].find("](") {
            let open = bang + 2 + close + 2;
            if let Some(endrel) = content[open..].find(')') {
                let src = &content[open..open + endrel];
                if !src.is_empty() && !src.contains(' ') {
                    refs.push(src.to_string());
                }
                from = open + endrel + 1;
                continue;
            }
        }
        from = bang + 2;
    }
    let mut from = 0;
    while let Some(rel) = content[from..].find("<img ") {
        let tag = from + rel;
        if let Some(srel) = content[tag..].find("src=\"") {
            let open = tag + srel + 5;
            if let Some(endrel) = content[open..].find('"') {
                refs.push(content[open..open + endrel].to_string());
            }
        }
        from = tag + 5;
    }
    refs
}

/// Resolve an image src against the doc's directory into a repo-relative
/// path (`/…` is repo-root-relative). Returns None when `..` escapes the
/// repo root.
fn resolve_repo_path(doc_dir: &str, src: &str) -> Option<String> {
    let base = if src.starts_with('/') { "" } else { doc_dir };
    let mut segments: Vec<&str> = Vec::new();
    let joined = base.to_string() + src;
    for seg in joined.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                if segments.pop().is_none() {
                    return None; // escapes the repo root
                }
            }
            s => segments.push(s),
        }
    }
    Some(segments.join("/"))
}

fn make_remote_doc_meta(
    source_id: &str,
    id: &str,
    title: String,
    category: String,
    hash: String,
    initial_status: TranslationStatus,
    now: &str,
) -> DocMeta {
    DocMeta {
        source_id: source_id.to_string(),
        id: id.to_string(),
        title,
        category,
        primary_hash: hash,
        // We just downloaded this content directly from upstream, so by
        // definition it's in sync at import time.
        last_checked_at: Some(now.to_string()),
        last_check_status: CheckStatus::Same,
        translation_status: initial_status,
        translated_at: None,
        translated_by: None,
    }
}

/// Language codes double as directory names on disk (`docs/<lang>/…`), so
/// anything but a plain code is rejected at this boundary — a `..` or a
/// separator in here would otherwise escape the source's doc tree.
fn validate_lang_code(code: &str, what: &str) -> Result<()> {
    if code.is_empty() || !code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        bail!("{what} '{code}' 不合法：只能包含字母、数字、'-' 和 '_'");
    }
    Ok(())
}

fn display_path(path: &str) -> String {
    let trimmed = path.trim_matches('/');
    if trimmed.is_empty() { "仓库根目录".to_string() } else { trimmed.to_string() }
}

fn normalized_prefix(path: &str) -> String {
    let trimmed = path.trim_matches('/');
    if trimmed.is_empty() {
        String::new()
    } else {
        format!("{trimmed}/")
    }
}

async fn fetch_tree_paths(client: &Client, spec: &RemoteSpec) -> Result<Vec<String>> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/git/trees/{}?recursive=1",
        spec.owner, spec.repo, spec.branch
    );
    let resp = client
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .with_context(|| format!("请求 {url} 失败"))?;
    if !resp.status().is_success() {
        bail!("GitHub API 返回 {}（{url}）", resp.status());
    }
    let body: TreeResponse = resp.json().await.context("解析 GitHub 仓库树响应失败")?;
    if body.truncated {
        bail!("仓库内容过多，一次请求无法列出全部文件（GitHub 截断了结果）");
    }
    Ok(body.tree.into_iter().filter(|e| e.kind == "blob").map(|e| e.path).collect())
}

/// Also used by `sync` to re-check a RemoteGit doc's upstream content.
pub(crate) fn raw_url_for_rel_path(spec: &RemoteSpec, rel_path: &str) -> String {
    let prefix = normalized_prefix(&spec.path);
    format!("https://raw.githubusercontent.com/{}/{}/{}/{prefix}{rel_path}", spec.owner, spec.repo, spec.branch)
}

pub(crate) async fn fetch_raw(client: &Client, url: &str) -> Result<String> {
    let resp = client.get(url).header("User-Agent", USER_AGENT).send().await.with_context(|| format!("请求 {url} 失败"))?;
    if !resp.status().is_success() {
        bail!("下载失败：{url} 返回 {}", resp.status());
    }
    resp.text().await.with_context(|| format!("读取 {url} 的响应内容失败"))
}

/// Binary counterpart of `fetch_raw` for assets (images). The bytes go
/// straight to disk without a UTF-8 round trip.
pub(crate) async fn fetch_raw_bytes(client: &Client, url: &str) -> Result<Vec<u8>> {
    let resp = client.get(url).header("User-Agent", USER_AGENT).send().await.with_context(|| format!("请求 {url} 失败"))?;
    if !resp.status().is_success() {
        bail!("下载失败：{url} 返回 {}", resp.status());
    }
    resp.bytes().await.map(|b| b.to_vec()).with_context(|| format!("读取 {url} 的响应内容失败"))
}

/// A manifest file directly under `spec.path` (no further nesting),
/// preferring `SUMMARY.md` > `_sidebar.md` > `README.md`, case-insensitive.
fn find_manifest_path(rel_paths: &[String]) -> Option<String> {
    for wanted in MANIFEST_FILE_NAMES {
        for p in rel_paths {
            if !p.contains('/') && p.to_lowercase() == *wanted {
                return Some(p.clone());
            }
        }
    }
    None
}

/// Same grouping rule as `local_import::build_fallback_categories`, minus
/// the "peek the first heading for a title" perk — the file content isn't
/// available yet at this point without an extra round trip per file, so
/// remote fallback titles are always a prettified id.
fn build_fallback_categories_from_ids(ids: &[String]) -> Vec<NavCategory> {
    let mut root_items = Vec::new();
    let mut grouped: HashMap<String, Vec<NavItem>> = HashMap::new();

    for id in ids {
        let title = prettify_id(id.rsplit('/').next().unwrap_or(id));
        match id.split_once('/') {
            None => root_items.push(NavItem { doc_id: id.clone(), title }),
            Some((top, _)) => grouped.entry(top.to_string()).or_default().push(NavItem { doc_id: id.clone(), title }),
        }
    }

    let mut categories = Vec::new();
    if !root_items.is_empty() {
        root_items.sort_by(|a, b| natural_cmp(&a.doc_id, &b.doc_id));
        categories.push(NavCategory { name: "未分类".to_string(), items: root_items });
    }
    let mut dir_names: Vec<String> = grouped.keys().cloned().collect();
    dir_names.sort_by(|a, b| natural_cmp(a, b));
    for dir_name in dir_names {
        let mut items = grouped.remove(&dir_name).unwrap_or_default();
        items.sort_by(|a, b| natural_cmp(&a.doc_id, &b.doc_id));
        categories.push(NavCategory { name: dir_name, items });
    }
    categories
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_manifest_path_prefers_summary_over_readme() {
        let paths = vec!["README.md".to_string(), "SUMMARY.md".to_string(), "intro.md".to_string()];
        assert_eq!(find_manifest_path(&paths), Some("SUMMARY.md".to_string()));
    }

    #[test]
    fn find_manifest_path_ignores_nested_readmes() {
        let paths = vec!["guides/README.md".to_string(), "intro.md".to_string()];
        assert_eq!(find_manifest_path(&paths), None);
    }

    #[test]
    fn fallback_categories_group_by_top_level_dir_with_natural_sort() {
        let ids = vec![
            "intro".to_string(),
            "guides/2-advanced".to_string(),
            "guides/10-appendix".to_string(),
            "guides/1-basics".to_string(),
        ];
        let categories = build_fallback_categories_from_ids(&ids);
        assert_eq!(categories[0].name, "未分类");
        assert_eq!(categories[0].items[0].doc_id, "intro");
        assert_eq!(categories[1].name, "guides");
        assert_eq!(
            categories[1].items.iter().map(|i| i.doc_id.as_str()).collect::<Vec<_>>(),
            vec!["guides/1-basics", "guides/2-advanced", "guides/10-appendix"],
        );
    }

    #[test]
    fn raw_url_handles_root_and_nested_paths() {
        let spec = RemoteSpec {
            owner: "acme".to_string(),
            repo: "docs".to_string(),
            branch: "main".to_string(),
            path: "packages/docs".to_string(),
        };
        assert_eq!(
            raw_url_for_rel_path(&spec, "intro.md"),
            "https://raw.githubusercontent.com/acme/docs/main/packages/docs/intro.md",
        );

        let root_spec = RemoteSpec { path: String::new(), ..spec };
        assert_eq!(raw_url_for_rel_path(&root_spec, "intro.md"), "https://raw.githubusercontent.com/acme/docs/main/intro.md");
    }

    #[test]
    fn collects_referenced_assets_from_doc_contents() {
        let docs = vec![
            ("intro".to_string(), "![a](images/arch.png)".to_string()),
            ("guides/getting-started".to_string(),
                "![b](./assets/x.jpg)\n![c](../diagrams/flow.svg)\n![d](https://example.com/ext.png)\n![e](/assets/root.png)\n![f](missing.png)".to_string()),
        ];
        let tree: HashSet<String> = [
            "images/arch.png", "guides/assets/x.jpg", "diagrams/flow.svg",
            "assets/root.png", "unrelated.png",
        ].iter().map(|s| s.to_string()).collect();

        let assets = collect_referenced_assets(&docs, &tree);

        assert_eq!(assets, [
            "images/arch.png", "guides/assets/x.jpg", "diagrams/flow.svg", "assets/root.png",
        ].iter().map(|s| s.to_string()).collect::<std::collections::BTreeSet<_>>());
    }

    #[test]
    fn image_refs_survives_multibyte_characters() {
        // Byte-wise scanning used to panic slicing mid-char (e.g. ⇐ arrows
        // in real Pi docs), which the anonymous rate limit had been hiding.
        let content = "映射 ⇐⇒ 说明\n![ok](images/a.png)\n结束 ⇒。";
        assert_eq!(image_refs(content), vec!["images/a.png".to_string()]);
    }

    #[test]
    fn collect_rejects_traversal_refs_and_still_returns_the_rest() {
        let docs = vec![
            ("doc".to_string(), "![a](../../etc/passwd.png)\n![b](ok.png)".to_string()),
        ];
        let tree: HashSet<String> = ["ok.png"].iter().map(|s| s.to_string()).collect();

        let assets = collect_referenced_assets(&docs, &tree);

        assert_eq!(assets, ["ok.png"].iter().map(|s| s.to_string()).collect::<std::collections::BTreeSet<_>>());
    }

    #[tokio::test]
    async fn imports_the_real_pi_docs_repo_end_to_end() {
        let app_dir = tempfile::tempdir().unwrap();
        let mut data = AppStateData::default();
        let spec = RemoteSpec {
            owner: "earendil-works".to_string(),
            repo: "pi".to_string(),
            branch: "main".to_string(),
            path: "packages/coding-agent/docs".to_string(),
        };

        let source = import_remote_source(app_dir.path(), &mut data, spec, Some("Pi (remote)".to_string()), "en".to_string(), None)
            .await
            .expect("network import against the real GitHub API should succeed");

        assert_eq!(source.kind, SourceKind::RemoteGit);
        assert_eq!(source.languages, vec!["en".to_string()]);
        // Not an exact count: this is a live upstream repo and genuinely
        // gains/loses docs over time (it did mid-development of this
        // feature!) — just sanity-check it's in the right ballpark.
        assert!(data.docs.len() >= 29, "expected at least 29 docs, got {}", data.docs.len());
        assert!(data.docs.iter().any(|d| d.id == "quickstart"));
        assert!(app_dir.path().join("sources").join(&source.id).join("docs/en/quickstart.md").exists());
        assert!(app_dir.path().join("sources").join(&source.id).join("docs/en/extensions.md").exists());

        // The upstream repo ships a Mintlify-style docs.json, not one of
        // our recognized manifest filenames, so this must fall back to
        // filesystem categorization: everything flat in one directory ->
        // a single "未分类" category, alphabetically sorted.
        let tree = crate::sources::read_manifest(app_dir.path(), &source.id).unwrap();
        assert_eq!(tree.categories.len(), 1);
        assert_eq!(tree.categories[0].name, "未分类");
        assert_eq!(tree.categories[0].items[0].doc_id, "compaction");
    }

    #[tokio::test]
    async fn importing_with_a_translation_target_makes_the_source_bilingual() {
        let app_dir = tempfile::tempdir().unwrap();
        let mut data = AppStateData::default();
        // Yazi's docs site: a real public Docusaurus repo, convenient here
        // because its docs/ folder is small and stable.
        let spec = RemoteSpec {
            owner: "yazi-rs".to_string(),
            repo: "yazi-rs.github.io".to_string(),
            branch: "main".to_string(),
            path: "docs".to_string(),
        };

        let source =
            import_remote_source(app_dir.path(), &mut data, spec, Some("Yazi".to_string()), "en".to_string(), Some("zh".to_string()))
                .await
                .expect("network import against the real GitHub API should succeed");

        // Registering a target language flips the source to bilingual with
        // every doc awaiting its first translation — which is exactly what
        // the UI keys its translation banners/buttons off.
        assert_eq!(source.languages, vec!["en".to_string(), "zh".to_string()]);
        assert_eq!(source.primary_language, "en");
        assert!(!data.docs.is_empty());
        assert!(data.docs.iter().all(|d| d.translation_status == TranslationStatus::NeverTranslated));
        let en_root = app_dir.path().join("sources").join(&source.id).join("docs/en");
        assert!(en_root.join("quick-start.md").exists());
        // No target-language files exist yet — translation creates them.
        assert!(!app_dir.path().join("sources").join(&source.id).join("docs/zh").exists());
    }

    #[tokio::test]
    async fn translation_target_validation_rejects_bad_input_before_any_network_io() {
        let app_dir = tempfile::tempdir().unwrap();
        let mut data = AppStateData::default();
        let spec = RemoteSpec {
            owner: "irrelevant".to_string(),
            repo: "irrelevant".to_string(),
            branch: "main".to_string(),
            path: String::new(),
        };

        let same_lang = import_remote_source(
            app_dir.path(), &mut data, spec.clone(), None, "en".to_string(), Some("en".to_string()),
        )
        .await
        .unwrap_err();
        assert!(same_lang.to_string().contains("不能与源语言相同"));

        // A hostile code would become a directory name under docs/, so
        // separators and ".." must be rejected at the boundary.
        let traversal = import_remote_source(
            app_dir.path(), &mut data, spec, None, "en".to_string(), Some("../evil".to_string()),
        )
        .await
        .unwrap_err();
        assert!(traversal.to_string().contains("不合法"));
    }

    #[test]
    fn validate_lang_code_accepts_plain_codes_only() {
        assert!(validate_lang_code("en", "").is_ok());
        assert!(validate_lang_code("zh-CN", "").is_ok());
        assert!(validate_lang_code("pt_br", "").is_ok());
        assert!(validate_lang_code("", "").is_err());
        assert!(validate_lang_code("a/b", "").is_err());
        assert!(validate_lang_code("..", "").is_err());
        assert!(validate_lang_code("zh CN", "").is_err());
    }
}
