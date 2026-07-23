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
pub async fn import_remote_source(
    dir: &Path,
    data: &mut AppStateData,
    spec: RemoteSpec,
    display_name: Option<String>,
    lang: String,
) -> Result<Source> {
    let client = Client::new();
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
    }

    let now = now_iso();
    let mentioned: HashSet<&str> = categories.iter().flat_map(|c| c.items.iter().map(|i| i.doc_id.as_str())).collect();

    let mut doc_metas = Vec::new();
    for cat in &categories {
        for item in &cat.items {
            let Some(hash) = hashes.get(&item.doc_id) else { continue };
            doc_metas.push(make_remote_doc_meta(&source_id, &item.doc_id, item.title.clone(), cat.name.clone(), hash.clone(), &now));
        }
    }
    for id in &md_ids {
        if mentioned.contains(id.as_str()) || manifest_doc_id.as_deref() == Some(id.as_str()) {
            continue;
        }
        let Some(hash) = hashes.get(id) else { continue };
        let title = prettify_id(id.rsplit('/').next().unwrap_or(id));
        doc_metas.push(make_remote_doc_meta(&source_id, id, title, "未分类".to_string(), hash.clone(), &now));
    }

    if doc_metas.is_empty() {
        bail!("导入失败：清单中提到的文档在仓库里都找不到对应文件");
    }

    write_manifest(dir, &source_id, &NavTree { source_id: source_id.clone(), categories })?;

    let source = Source {
        id: source_id.clone(),
        name: name.clone(),
        kind: SourceKind::RemoteGit,
        languages: vec![lang.clone()],
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
    });

    Ok(source)
}

fn make_remote_doc_meta(source_id: &str, id: &str, title: String, category: String, hash: String, now: &str) -> DocMeta {
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
        translation_status: TranslationStatus::NotApplicable,
        translated_at: None,
        translated_by: None,
    }
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

        let source = import_remote_source(app_dir.path(), &mut data, spec, Some("Pi (remote)".to_string()), "en".to_string())
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
}
