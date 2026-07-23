//! Checking a source's documents against their upstream copy and applying
//! updates. "Upstream" means different things per source kind: for
//! `RemoteGit` (and `Seed`, which also carries a `RemoteSpec`) it's a fresh
//! `raw.githubusercontent.com` fetch; for `LocalFolder` it's a re-read of
//! the original imported path (`Source.local_path`). Sources with neither
//! don't support checking.

use std::path::Path;

use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::Serialize;

use crate::hashing::sha256_hex;
use crate::local_import::lang_root;
use crate::remote_import::{fetch_raw, raw_url_for_rel_path};
use crate::sources::doc_content_path;
use crate::state::{
    now_iso, AppStateData, CheckStatus, DocMeta, LogEntry, LogKind, Source, TranslationStatus,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckSummary {
    pub checked: usize,
    pub changed: Vec<String>,
    pub errored: Vec<String>,
}

/// Re-check every doc in `source_id` against its upstream copy, updating
/// each `DocMeta.lastCheckStatus`/`lastCheckedAt` and logging a summary.
/// Does not persist `data` — the caller saves once the lock is released.
pub async fn check_updates(data: &mut AppStateData, source_id: &str) -> Result<CheckSummary> {
    let source = find_source(data, source_id)?;
    let doc_ids: Vec<String> =
        data.docs.iter().filter(|d| d.source_id == source_id).map(|d| d.id.clone()).collect();

    let client = Client::new();
    let now = now_iso();
    let mut checked = 0usize;
    let mut changed = Vec::new();
    let mut errored = Vec::new();

    for doc_id in &doc_ids {
        let outcome = fetch_upstream_content(&client, &source, doc_id).await;
        let doc = data
            .docs
            .iter_mut()
            .find(|d| d.source_id == source_id && d.id == *doc_id)
            .expect("doc_id came from this same source's doc list");
        doc.last_checked_at = Some(now.clone());

        match outcome {
            Ok(content) => {
                checked += 1;
                if sha256_hex(&content) == doc.primary_hash {
                    doc.last_check_status = CheckStatus::Same;
                } else {
                    doc.last_check_status = CheckStatus::Changed;
                    changed.push(doc_id.clone());
                }
            }
            Err(e) => {
                doc.last_check_status = CheckStatus::Error;
                errored.push(doc_id.clone());
                eprintln!("check_updates: failed to check {source_id}/{doc_id}: {e}");
            }
        }
    }

    data.log.push(LogEntry {
        id: format!("check-{source_id}-{now}"),
        ts: now,
        source_id: source_id.to_string(),
        doc_id: None,
        kind: LogKind::Check,
        detail: format!("Checked {checked} docs: {} changed, {} errored", changed.len(), errored.len()),
    });

    Ok(CheckSummary { checked, changed, errored })
}

/// Overwrite `doc_id`'s primary-language content with a fresh upstream
/// fetch, update its hash/check status, and (for bilingual sources that
/// already had a translation) mark it `Pending` re-translation. Does not
/// persist `data` — the caller saves once the lock is released.
pub async fn apply_update(dir: &Path, data: &mut AppStateData, source_id: &str, doc_id: &str) -> Result<DocMeta> {
    let source = find_source(data, source_id)?;
    let client = Client::new();
    let content = fetch_upstream_content(&client, &source, doc_id)
        .await
        .with_context(|| format!("拉取 '{doc_id}' 的最新内容失败"))?;

    let dest_path = doc_content_path(dir, source_id, &source.primary_language, doc_id);
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&dest_path, &content).with_context(|| format!("writing {dest_path:?}"))?;

    let now = now_iso();
    let doc = data
        .docs
        .iter_mut()
        .find(|d| d.source_id == source_id && d.id == doc_id)
        .ok_or_else(|| anyhow::anyhow!("未知的文档 '{doc_id}'"))?;
    doc.primary_hash = sha256_hex(&content);
    doc.last_checked_at = Some(now.clone());
    doc.last_check_status = CheckStatus::Same;
    if doc.translation_status == TranslationStatus::Translated {
        doc.translation_status = TranslationStatus::Pending;
    }
    let updated = doc.clone();

    data.log.push(LogEntry {
        id: format!("apply-{source_id}-{doc_id}-{now}"),
        ts: now,
        source_id: source_id.to_string(),
        doc_id: Some(doc_id.to_string()),
        kind: LogKind::ApplyUpdate,
        detail: format!("Applied upstream update to '{doc_id}'"),
    });

    Ok(updated)
}

fn find_source(data: &AppStateData, source_id: &str) -> Result<Source> {
    data.sources
        .iter()
        .find(|s| s.id == source_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("未知的来源 '{source_id}'"))
}

async fn fetch_upstream_content(client: &Client, source: &Source, doc_id: &str) -> Result<String> {
    if let Some(remote) = &source.remote {
        let url = raw_url_for_rel_path(remote, &format!("{doc_id}.md"));
        fetch_raw(client, &url).await
    } else if let Some(local_path) = &source.local_path {
        let root = lang_root(Path::new(local_path), &source.languages, &source.primary_language);
        let file_path = root.join(format!("{doc_id}.md"));
        std::fs::read_to_string(&file_path).with_context(|| format!("reading {file_path:?}"))
    } else {
        bail!("此来源既没有远程地址也没有本地路径，无法检查更新")
    }
}

/// If `translation.checkOnStartup` is enabled, best-effort check every
/// source that supports it (has a `remote` or `local_path`) in the
/// background. A failure on one source (e.g. no network) is logged and
/// skipped — this is a nice-to-have background refresh, never something
/// that should block or interrupt app startup.
pub async fn run_startup_checks(dir: &Path, data: &mut AppStateData) {
    let cfg = match crate::config::load_config(dir) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("startup check: failed to load config: {e}");
            return;
        }
    };
    if !cfg.translation.check_on_startup {
        return;
    }

    let checkable_ids: Vec<String> = data
        .sources
        .iter()
        .filter(|s| s.remote.is_some() || s.local_path.is_some())
        .map(|s| s.id.clone())
        .collect();

    for source_id in checkable_ids {
        if let Err(e) = check_updates(data, &source_id).await {
            eprintln!("startup check: failed for source '{source_id}': {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_import::import_local_folder;

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    #[tokio::test]
    async fn detects_and_applies_a_local_upstream_change() {
        let app_dir = tempfile::tempdir().unwrap();
        let upstream = tempfile::tempdir().unwrap();
        write(&upstream.path().join("intro.md"), "# Intro\n\nOriginal content.");

        let mut data = AppStateData::default();
        let source =
            import_local_folder(app_dir.path(), &mut data, upstream.path(), Some("Notes".to_string())).unwrap();

        // Nothing has changed yet.
        let summary = check_updates(&mut data, &source.id).await.unwrap();
        assert_eq!(summary.checked, 1);
        assert!(summary.changed.is_empty());
        let doc = data.docs.iter().find(|d| d.id == "intro").unwrap();
        assert_eq!(doc.last_check_status, CheckStatus::Same);

        // Now mutate the "upstream" folder directly, bypassing our copy.
        write(&upstream.path().join("intro.md"), "# Intro\n\nUpdated content!");
        let summary = check_updates(&mut data, &source.id).await.unwrap();
        assert_eq!(summary.changed, vec!["intro".to_string()]);
        let doc = data.docs.iter().find(|d| d.id == "intro").unwrap();
        assert_eq!(doc.last_check_status, CheckStatus::Changed);

        // Applying pulls the new content in and flips the status back to Same.
        let updated = apply_update(app_dir.path(), &mut data, &source.id, "intro").await.unwrap();
        assert_eq!(updated.last_check_status, CheckStatus::Same);
        let on_disk = std::fs::read_to_string(
            app_dir.path().join("sources").join(&source.id).join("docs/default/intro.md"),
        )
        .unwrap();
        assert!(on_disk.contains("Updated content!"));
    }

    #[test]
    fn find_source_errors_on_unknown_id() {
        let data = AppStateData::default();
        let err = find_source(&data, "nope").unwrap_err();
        assert!(err.to_string().contains("未知的来源"));
    }

    #[tokio::test]
    async fn checking_the_real_remote_pi_source_runs_without_error() {
        let app_dir = tempfile::tempdir().unwrap();
        let mut data = AppStateData::default();
        crate::sources::ensure_seed_source(app_dir.path(), &mut data).unwrap();

        // The bundled Pi source carries a real RemoteSpec pointing at the
        // upstream repo, so this genuinely hits the network.
        let summary = check_updates(&mut data, "pi").await.unwrap();
        assert_eq!(summary.checked, 29);
        assert!(summary.errored.is_empty());
    }

    #[tokio::test]
    async fn startup_check_is_a_noop_when_explicitly_disabled() {
        let app_dir = tempfile::tempdir().unwrap();
        let mut data = AppStateData::default();
        crate::sources::ensure_seed_source(app_dir.path(), &mut data).unwrap();

        let mut cfg = crate::config::AppConfig::default();
        cfg.translation.check_on_startup = false;
        crate::config::save_config_atomic(app_dir.path(), &cfg).unwrap();

        run_startup_checks(app_dir.path(), &mut data).await;
        assert!(data.docs.iter().all(|d| d.last_checked_at.is_none()));
    }

    #[tokio::test]
    async fn startup_check_runs_local_sources_by_default() {
        let app_dir = tempfile::tempdir().unwrap();
        let upstream = tempfile::tempdir().unwrap();
        write(&upstream.path().join("intro.md"), "# Intro\n\nHello.");

        let mut data = AppStateData::default();
        let source = import_local_folder(app_dir.path(), &mut data, upstream.path(), None).unwrap();

        // No config.json on disk at all -> defaults apply -> checkOnStartup
        // defaults to true (matches the config.json example in the plan).
        run_startup_checks(app_dir.path(), &mut data).await;
        let doc = data.docs.iter().find(|d| d.source_id == source.id).unwrap();
        assert_eq!(doc.last_check_status, CheckStatus::Same);
        assert!(doc.last_checked_at.is_some());
    }
}
