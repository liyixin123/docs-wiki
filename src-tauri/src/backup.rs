//! Exporting/importing a portable backup of app *metadata* (`state.json` +
//! `config.json`) — not the actual document content, which lives under
//! `sources/<id>/docs/` and isn't part of this bundle. Restoring a backup
//! onto a machine without matching `sources/` directories will show
//! sources whose docs can't be read until they're re-imported or re-synced.

use std::io::{Read, Write};
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::{self, AppConfig};
use crate::state::{self, AppStateData, CheckStatus, DocMeta, Source, SourceKind, TranslationStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Backup {
    state: AppStateData,
    config: AppConfig,
}

/// Bundle `state.json` + `config.json` into one JSON file at `dest_path`.
///
/// Note this may include a literal API key if the user typed one directly
/// into settings instead of using a `$ENV_VAR`/`!command` expression —
/// the frontend warns about this before exporting.
pub fn export_backup(dir: &Path, dest_path: &Path) -> Result<()> {
    let backup = Backup {
        state: state::load_state(dir)?.unwrap_or_default(),
        config: config::load_config(dir)?,
    };
    let json = serde_json::to_string_pretty(&backup).context("serializing backup")?;
    std::fs::write(dest_path, json).with_context(|| format!("writing {dest_path:?}"))?;
    Ok(())
}

/// Full-library share bundle (".docswiki", a zip): carries every
/// `sources/<id>/` tree (docs, assets, manifest, nav) plus a trimmed state
/// (sources + docs metadata only — no config, no log, no history snapshots).
/// Importing merges by source id: new ids land on disk and in state,
/// existing ids are skipped untouched.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LibraryShare {
    sources: Vec<Source>,
    docs: Vec<DocMeta>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryImportSummary {
    pub imported: usize,
    pub skipped: usize,
}

const LIBRARY_JSON: &str = "library.json";

/// Write a `.docswiki` zip of the whole library to `dest_path`. Excludes
/// `.history` snapshot dirs (personal) — everything else under
/// `sources/<id>/` ships verbatim.
pub fn export_library(dir: &Path, dest_path: &Path) -> Result<()> {
    let data = state::load_state(dir)?.unwrap_or_default();
    if data.sources.is_empty() {
        bail!("当前没有任何文档来源，没有可导出的内容");
    }
    let share = LibraryShare { sources: data.sources.clone(), docs: data.docs.clone() };

    let file = std::fs::File::create(dest_path).with_context(|| format!("creating {dest_path:?}"))?;
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    zip.start_file(LIBRARY_JSON, options)?;
    zip.write_all(serde_json::to_string_pretty(&share)?.as_bytes())?;

    let sources_root = dir.join("sources");
    for source in &data.sources {
        let source_dir = sources_root.join(&source.id);
        if !source_dir.is_dir() {
            continue; // metadata without content; nothing to ship
        }
        let mut stack = vec![source_dir.clone()];
        while let Some(current) = stack.pop() {
            for entry in std::fs::read_dir(&current)? {
                let entry = entry?;
                let path = entry.path();
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
                if name == ".history" {
                    continue;
                }
                if path.is_dir() {
                    stack.push(path);
                } else {
                    let rel = path.strip_prefix(dir).with_context(|| format!("{path:?} outside app dir"))?;
                    let name = rel.to_string_lossy().replace('\\', "/");
                    zip.start_file(name.clone(), options)?;
                    std::io::copy(&mut std::fs::File::open(&path)?, &mut zip)?;
                }
            }
        }
    }
    zip.finish()?;
    Ok(())
}

/// Merge a `.docswiki` bundle into `dir`/`data`: for each bundled source
/// whose id isn't already present, extract its `sources/<id>/` tree and
/// append its metadata. Returns how many sources landed vs were skipped.
pub fn import_library(dir: &Path, data: &mut AppStateData, src_path: &Path) -> Result<LibraryImportSummary> {
    let file = std::fs::File::open(src_path).with_context(|| format!("opening {src_path:?}"))?;
    let mut zip = zip::ZipArchive::new(file).context("解析文档库文件失败（不是有效的 .docswiki 文件？）")?;

    let mut library_json = String::new();
    zip.by_name(LIBRARY_JSON)
        .context("文档库文件缺少 library.json")?
        .read_to_string(&mut library_json)?;
    let share: LibraryShare =
        serde_json::from_str(&library_json).context("解析文档库元数据失败")?;

    let existing: std::collections::HashSet<String> =
        data.sources.iter().map(|s| s.id.clone()).collect();
    let mut imported = 0usize;
    let mut skipped = 0usize;

    for source in share.sources {
        if existing.contains(&source.id) {
            skipped += 1;
            continue;
        }
        let prefix = format!("sources/{}/", source.id);
        let source_dir = dir.join("sources").join(&source.id);
        std::fs::create_dir_all(&source_dir)?;
        let mut i = 0usize;
        while i < zip.len() {
            let mut entry = zip.by_index(i)?;
            let name = entry.name().to_string();
            let is_dir = entry.is_dir();
            if name.starts_with(&prefix) {
                let rel = name.strip_prefix(&prefix).unwrap();
                // Defensive: refuse path escapes inside the bundle.
                if rel.split('/').any(|seg| seg == ".." || seg.is_empty() && !is_dir) {
                    bail!("文档库包含非法路径: {name}");
                }
                let dest = source_dir.join(rel.trim_end_matches('/'));
                if is_dir {
                    std::fs::create_dir_all(&dest)?;
                } else if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                    let mut bytes = Vec::new();
                    entry.read_to_end(&mut bytes)?;
                    std::fs::write(&dest, bytes)?;
                }
            }
            i += 1;
        }
        data.docs.extend(share.docs.iter().filter(|d| d.source_id == source.id).cloned());
        data.sources.push(source);
        imported += 1;
    }
    Ok(LibraryImportSummary { imported, skipped })
}

/// Restore `state.json`/`config.json` from a backup file, returning the
/// restored state so the caller can also update the in-memory `AppState`
/// without requiring a restart.
pub fn import_backup(dir: &Path, src_path: &Path) -> Result<AppStateData> {
    let content = std::fs::read_to_string(src_path).with_context(|| format!("reading {src_path:?}"))?;
    let backup: Backup = serde_json::from_str(&content).context("解析备份文件失败：格式不正确")?;

    state::save_state_atomic(dir, &backup.state)?;
    config::save_config_atomic(dir, &backup.config)?;
    Ok(backup.state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::ensure_seed_source;

    #[test]
    fn export_then_import_round_trips_state_and_config() {
        let original_dir = tempfile::tempdir().unwrap();
        let mut data = AppStateData::default();
        ensure_seed_source(original_dir.path(), &mut data).unwrap();
        state::save_state_atomic(original_dir.path(), &data).unwrap();

        let mut cfg = AppConfig::default();
        cfg.active_provider = Some("anthropic".to_string());
        config::save_config_atomic(original_dir.path(), &cfg).unwrap();

        let backup_file = original_dir.path().join("backup.json");
        export_backup(original_dir.path(), &backup_file).unwrap();

        // Restore onto a completely different (empty) app data directory.
        let restore_dir = tempfile::tempdir().unwrap();
        let restored = import_backup(restore_dir.path(), &backup_file).unwrap();

        assert_eq!(restored.sources.len(), 1);
        assert_eq!(restored.sources[0].id, "pi");

        let restored_cfg = config::load_config(restore_dir.path()).unwrap();
        assert_eq!(restored_cfg.active_provider, Some("anthropic".to_string()));
    }

    #[test]
    fn import_rejects_a_malformed_backup_file() {
        let dir = tempfile::tempdir().unwrap();
        let bogus = dir.path().join("not-a-backup.json");
        std::fs::write(&bogus, "{ this is not valid json").unwrap();

        let err = import_backup(dir.path(), &bogus).unwrap_err();
        assert!(err.to_string().contains("解析备份文件失败"));
    }

    #[test]
    fn export_defaults_gracefully_when_nothing_exists_yet() {
        let empty_dir = tempfile::tempdir().unwrap();
        let dest = empty_dir.path().join("backup.json");
        // No state.json or config.json written yet — should still produce a
        // valid (empty-defaults) backup rather than erroring.
        export_backup(empty_dir.path(), &dest).unwrap();
        assert!(dest.exists());
    }

    fn seed_library_dir(dir: &std::path::Path) {
        let mut data = AppStateData::default();
        data.sources.push(Source {
            id: "shape-up".to_string(),
            name: "Shape Up".to_string(),
            kind: SourceKind::LocalFolder,
            languages: vec!["en".to_string(), "zh".to_string()],
            primary_language: "en".to_string(),
            remote: None,
            local_path: Some("/wherever".to_string()),
            order_override: vec![],
            created_at: state::now_iso(),
            updated_at: state::now_iso(),
        });
        data.docs.push(DocMeta {
            source_id: "shape-up".to_string(),
            id: "intro".to_string(),
            title: "Intro".to_string(),
            category: "".to_string(),
            primary_hash: "h".to_string(),
            last_checked_at: None,
            last_check_status: CheckStatus::Same,
            translation_status: TranslationStatus::Translated,
            translated_at: Some(state::now_iso()),
            translated_by: Some("anthropic".to_string()),
        });
        state::save_state_atomic(dir, &data).unwrap();
        let src_root = dir.join("sources/shape-up");
        std::fs::create_dir_all(src_root.join("docs/en")).unwrap();
        std::fs::create_dir_all(src_root.join("docs/zh")).unwrap();
        std::fs::write(src_root.join("docs/en/intro.md"), "# Intro").unwrap();
        std::fs::write(src_root.join("docs/zh/intro.md"), "# 引言").unwrap();
        std::fs::write(src_root.join("manifest.json"), "{}").unwrap();
        std::fs::create_dir_all(src_root.join(".history/zh")).unwrap();
        std::fs::write(src_root.join(".history/zh/old.md"), "stale").unwrap();
    }

    #[test]
    fn library_roundtrip_moves_content_state_and_translations() {
        let author_dir = tempfile::tempdir().unwrap();
        seed_library_dir(author_dir.path());
        let share_file = author_dir.path().join("lib.docswiki");
        export_library(author_dir.path(), &share_file).unwrap();

        // Import onto a fresh machine: empty dir, empty state.
        let reader_dir = tempfile::tempdir().unwrap();
        let mut data = AppStateData::default();
        let summary = import_library(reader_dir.path(), &mut data, &share_file).unwrap();
        assert_eq!((summary.imported, summary.skipped), (1, 0));

        assert_eq!(data.sources.len(), 1);
        assert_eq!(data.sources[0].id, "shape-up");
        assert_eq!(data.docs[0].translation_status, TranslationStatus::Translated);
        let root = reader_dir.path().join("sources/shape-up");
        assert_eq!(std::fs::read_to_string(root.join("docs/en/intro.md")).unwrap(), "# Intro");
        assert_eq!(std::fs::read_to_string(root.join("docs/zh/intro.md")).unwrap(), "# 引言");
        assert!(root.join("manifest.json").exists());
        // history snapshots are personal, not shared
        assert!(!root.join(".history").exists());
    }

    #[test]
    fn library_import_skips_existing_source_ids_and_keeps_local_content() {
        let author_dir = tempfile::tempdir().unwrap();
        seed_library_dir(author_dir.path());
        let share_file = author_dir.path().join("lib.docswiki");
        export_library(author_dir.path(), &share_file).unwrap();

        // Reader already has the same source id with its own content.
        let reader_dir = tempfile::tempdir().unwrap();
        seed_library_dir(reader_dir.path());
        std::fs::write(
            reader_dir.path().join("sources/shape-up/docs/en/intro.md"),
            "# Local version",
        )
        .unwrap();
        let mut data = state::load_state(reader_dir.path()).unwrap().unwrap();
        let before = data.clone();

        let summary = import_library(reader_dir.path(), &mut data, &share_file).unwrap();
        assert_eq!((summary.imported, summary.skipped), (0, 1));
        let data_before = serde_json::to_string(&before).unwrap();
        let data_after = serde_json::to_string(&data).unwrap();
        assert_eq!(data_after, data_before);
        assert_eq!(
            std::fs::read_to_string(reader_dir.path().join("sources/shape-up/docs/en/intro.md"))
                .unwrap(),
            "# Local version"
        );
    }
}
