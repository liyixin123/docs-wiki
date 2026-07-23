//! Source management: bootstrapping the bundled Pi seed source on first run,
//! removing a source, plus shared helpers (manifest/doc-content paths) used
//! by every source kind. Local-folder import lives in `local_import.rs`;
//! remote-git import lives in `remote_import.rs`.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use include_dir::{include_dir, Dir};

use crate::hashing::sha256_hex;
use crate::nav::{parse_link_manifest, prettify_id, NavCategory, NavTree};
use crate::state::{
    now_iso, AppStateData, DocMeta, LogEntry, LogKind, RemoteSpec, Source, SourceKind,
    TranslationStatus,
};

/// The Pi docs seed content, embedded into the binary at compile time so
/// the app never depends on a `seed-docs/` folder existing next to it at
/// runtime.
static SEED_DOCS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../seed-docs");

const PI_SOURCE_ID: &str = "pi";

/// If the "pi" seed source hasn't been imported yet, copy its bundled
/// markdown to `<dir>/sources/pi/docs/{en,zh}`, derive the nav manifest from
/// `zh/README.md`, register the `Source` + `DocMeta` entries, and persist
/// everything. Idempotent: does nothing if the source is already present.
pub fn ensure_seed_source(dir: &Path, data: &mut AppStateData) -> Result<bool> {
    if data.sources.iter().any(|s| s.id == PI_SOURCE_ID) {
        return Ok(false);
    }

    let en_dir = SEED_DOCS
        .get_dir("en")
        .context("embedded seed-docs missing en/ directory")?;
    let zh_dir = SEED_DOCS
        .get_dir("zh")
        .context("embedded seed-docs missing zh/ directory")?;

    let docs_en = dir.join("sources").join(PI_SOURCE_ID).join("docs/en");
    let docs_zh = dir.join("sources").join(PI_SOURCE_ID).join("docs/zh");
    std::fs::create_dir_all(&docs_en)?;
    std::fs::create_dir_all(&docs_zh)?;

    let now = now_iso();
    let mut doc_metas = Vec::new();

    for file in en_dir.files() {
        let Some(file_name) = file.path().file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let content = file
            .contents_utf8()
            .with_context(|| format!("seed-docs/en/{file_name} is not valid UTF-8"))?;
        std::fs::write(docs_en.join(file_name), content)
            .with_context(|| format!("writing docs/en/{file_name}"))?;

        let id = file_name.trim_end_matches(".md").to_string();
        doc_metas.push((id, sha256_hex(content)));
    }

    let mut readme_content: Option<&str> = None;
    for file in zh_dir.files() {
        let Some(file_name) = file.path().file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let content = file
            .contents_utf8()
            .with_context(|| format!("seed-docs/zh/{file_name} is not valid UTF-8"))?;
        if file_name.eq_ignore_ascii_case("README.md") {
            readme_content = Some(content);
            // README.md is the nav manifest source, not a browsable doc itself.
            continue;
        }
        std::fs::write(docs_zh.join(file_name), content)
            .with_context(|| format!("writing docs/zh/{file_name}"))?;
    }

    let categories = readme_content
        .map(parse_link_manifest)
        .context("seed-docs/zh/README.md missing; cannot derive nav order")?;

    let nav_tree = NavTree {
        source_id: PI_SOURCE_ID.to_string(),
        categories: categories.clone(),
    };
    write_manifest(dir, PI_SOURCE_ID, &nav_tree)?;

    // Build DocMeta in manifest order (category + title), falling back to
    // "未分类" for any en doc the manifest didn't mention (keeps a corrupt
    // or incomplete README from silently dropping documents).
    let mut ordered = build_doc_metas(&categories, &doc_metas, &now);
    let mentioned: std::collections::HashSet<&str> =
        categories.iter().flat_map(|c| c.items.iter().map(|i| i.doc_id.as_str())).collect();
    for (id, hash) in &doc_metas {
        if !mentioned.contains(id.as_str()) {
            ordered.push(make_doc_meta(id.clone(), prettify_id(id), "未分类".to_string(), hash.clone(), &now));
        }
    }

    let source = Source {
        id: PI_SOURCE_ID.to_string(),
        name: "Pi Coding Agent 文档".to_string(),
        kind: SourceKind::Seed,
        languages: vec!["en".to_string(), "zh".to_string()],
        primary_language: "en".to_string(),
        remote: Some(RemoteSpec {
            owner: "earendil-works".to_string(),
            repo: "pi".to_string(),
            branch: "main".to_string(),
            path: "packages/coding-agent/docs".to_string(),
        }),
        local_path: None,
        order_override: Vec::new(),
        created_at: now.clone(),
        updated_at: now.clone(),
    };

    data.sources.push(source);
    data.docs.extend(ordered);
    data.log.push(LogEntry {
        id: format!("import-{PI_SOURCE_ID}-{now}"),
        ts: now.clone(),
        source_id: PI_SOURCE_ID.to_string(),
        doc_id: None,
        kind: LogKind::Import,
        detail: format!("Imported bundled seed source '{PI_SOURCE_ID}' ({} docs)", doc_metas.len()),
    });

    Ok(true)
}

fn build_doc_metas(
    categories: &[NavCategory],
    hashes: &[(String, String)],
    now: &str,
) -> Vec<DocMeta> {
    let mut out = Vec::new();
    for cat in categories {
        for item in &cat.items {
            let Some((_, hash)) = hashes.iter().find(|(id, _)| id == &item.doc_id) else {
                continue; // manifest mentions a doc with no corresponding en/*.md; skip it
            };
            out.push(make_doc_meta(
                item.doc_id.clone(),
                item.title.clone(),
                cat.name.clone(),
                hash.clone(),
                now,
            ));
        }
    }
    out
}

fn make_doc_meta(id: String, title: String, category: String, hash: String, now: &str) -> DocMeta {
    DocMeta {
        source_id: PI_SOURCE_ID.to_string(),
        id,
        title,
        category,
        primary_hash: hash,
        last_checked_at: None,
        last_check_status: crate::state::CheckStatus::Unknown,
        translation_status: TranslationStatus::Translated,
        translated_at: Some(now.to_string()),
        translated_by: Some("pre-translated seed content".to_string()),
    }
}

pub fn manifest_path(dir: &Path, source_id: &str) -> PathBuf {
    dir.join("sources").join(source_id).join("manifest.json")
}

pub fn write_manifest(dir: &Path, source_id: &str, tree: &NavTree) -> Result<()> {
    let path = manifest_path(dir, source_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(tree)?;
    std::fs::write(&path, json).with_context(|| format!("writing {path:?}"))?;
    Ok(())
}

/// Read a source's nav manifest back from disk.
pub fn read_manifest(dir: &Path, source_id: &str) -> Result<NavTree> {
    let path = manifest_path(dir, source_id);
    let content = std::fs::read_to_string(&path).with_context(|| format!("reading {path:?}"))?;
    let tree: NavTree = serde_json::from_str(&content).with_context(|| format!("parsing {path:?}"))?;
    Ok(tree)
}

pub fn doc_content_path(dir: &Path, source_id: &str, lang: &str, id: &str) -> PathBuf {
    dir.join("sources")
        .join(source_id)
        .join("docs")
        .join(lang)
        .join(format!("{id}.md"))
}

/// Remove a source entirely, regardless of kind: delete its private data
/// directory on disk and drop its `Source`/`DocMeta`/`LogEntry` records.
/// Deletes the directory *before* mutating `data`, so a failed delete
/// leaves the source intact rather than the app pointing at missing files.
/// Does not persist `data` itself — the caller saves once the lock is released.
pub fn remove_source(dir: &Path, data: &mut AppStateData, source_id: &str) -> Result<()> {
    if !data.sources.iter().any(|s| s.id == source_id) {
        bail!("未知的来源 '{source_id}'");
    }

    let source_dir = dir.join("sources").join(source_id);
    if source_dir.exists() {
        std::fs::remove_dir_all(&source_dir).with_context(|| format!("removing {source_dir:?}"))?;
    }

    data.sources.retain(|s| s.id != source_id);
    data.docs.retain(|d| d.source_id != source_id);
    data.log.retain(|l| l.source_id != source_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppStateData;

    #[test]
    fn seed_import_is_idempotent_and_populates_state() {
        let tmp = tempfile::tempdir().unwrap();
        let mut data = AppStateData::default();

        let did_import = ensure_seed_source(tmp.path(), &mut data).unwrap();
        assert!(did_import);
        assert_eq!(data.sources.len(), 1);
        assert_eq!(data.sources[0].id, PI_SOURCE_ID);
        assert!(!data.docs.is_empty());
        assert!(tmp.path().join("sources/pi/docs/en/quickstart.md").exists());
        assert!(tmp.path().join("sources/pi/docs/zh/quickstart.md").exists());
        assert!(!tmp.path().join("sources/pi/docs/zh/README.md").exists());
        assert!(manifest_path(tmp.path(), PI_SOURCE_ID).exists());

        let doc_count_before = data.docs.len();
        let did_import_again = ensure_seed_source(tmp.path(), &mut data).unwrap();
        assert!(!did_import_again);
        assert_eq!(data.docs.len(), doc_count_before);
    }

    #[test]
    fn manifest_matches_readme_category_order() {
        let tmp = tempfile::tempdir().unwrap();
        let mut data = AppStateData::default();
        ensure_seed_source(tmp.path(), &mut data).unwrap();

        let tree = read_manifest(tmp.path(), PI_SOURCE_ID).unwrap();
        assert!(!tree.categories.is_empty());
        assert_eq!(tree.categories[0].name, "入门");
        assert_eq!(tree.categories[0].items[0].doc_id, "index");
    }

    #[test]
    fn remove_source_deletes_dir_and_state_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let mut data = AppStateData::default();
        ensure_seed_source(tmp.path(), &mut data).unwrap();
        assert!(tmp.path().join("sources").join(PI_SOURCE_ID).exists());

        remove_source(tmp.path(), &mut data, PI_SOURCE_ID).unwrap();

        assert!(!tmp.path().join("sources").join(PI_SOURCE_ID).exists());
        assert!(data.sources.is_empty());
        assert!(data.docs.is_empty());
    }

    #[test]
    fn remove_source_errors_on_unknown_id() {
        let tmp = tempfile::tempdir().unwrap();
        let mut data = AppStateData::default();
        let err = remove_source(tmp.path(), &mut data, "does-not-exist").unwrap_err();
        assert!(err.to_string().contains("未知的来源"));
    }
}
