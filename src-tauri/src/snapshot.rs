//! Content snapshots: before `apply_update`/`translate` overwrites a doc
//! file, the old content is copied under `sources/<id>/.history/<lang>/` so
//! the history UI can diff "what changed". Snapshots travel with the source
//! directory (so `remove_source` cleans them up) and are intentionally not
//! part of `backup.rs`'s metadata-only export — restoring a backup onto a
//! fresh machine leaves the logs pointing at snapshots that no longer exist,
//! which `read_snapshot` surfaces as a friendly error.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::state::SnapshotRef;

/// `<dir>/sources/<source_id>/.history/<lang>/` — every snapshot for one
/// language of one source lives here, alongside that source's `docs/`.
pub fn snapshot_dir(dir: &Path, source_id: &str, lang: &str) -> PathBuf {
    dir.join("sources").join(source_id).join(".history").join(lang)
}

/// Turn an RFC3339 timestamp (as produced by `state::now_iso`, e.g.
/// `2026-07-23T02:48:07.123456789+00:00`) into a filename-safe, sortable key:
/// drop the fractional seconds and the `+00:00` UTC offset, and replace `:`
/// (illegal in Windows file names) with `-`. Result: `2026-07-23T02-48-07`.
///
/// Only the second resolution is kept, so two snapshots of the same doc within
/// one second would collide — acceptable for a manually-triggered update flow.
fn sanitize_ts_for_filename(now: &str) -> String {
    let without_offset = now.split('+').next().unwrap_or(now);
    let without_frac = without_offset.split('.').next().unwrap_or(without_offset);
    without_frac.replace(':', "-")
}

/// Write `content` as a new snapshot and return a [`SnapshotRef`] pointing at
/// it. `content` may be empty. The file name embeds `doc_id` and the sanitized
/// timestamp so it self-describes what/when it is on disk.
pub fn save_snapshot(
    dir: &Path,
    source_id: &str,
    lang: &str,
    doc_id: &str,
    now: &str,
    content: &str,
) -> Result<SnapshotRef> {
    let folder = snapshot_dir(dir, source_id, lang);
    std::fs::create_dir_all(&folder).with_context(|| format!("creating snapshot dir {folder:?}"))?;
    let file = format!("{}.{}.md", doc_id, sanitize_ts_for_filename(now));
    let path = folder.join(&file);
    std::fs::write(&path, content).with_context(|| format!("writing snapshot {path:?}"))?;
    Ok(SnapshotRef { lang: lang.to_string(), file })
}

/// Read a snapshot back. A missing file yields an `Err` whose message names the
/// likely cause, so the `get_diff` command can surface it to the user rather
/// than crashing.
pub fn read_snapshot(dir: &Path, source_id: &str, snap: &SnapshotRef) -> Result<String> {
    let path = snapshot_dir(dir, source_id, &snap.lang).join(&snap.file);
    std::fs::read_to_string(&path).with_context(|| {
        format!("snapshot {path:?} 不在本机，可能因导入备份或手动清理而丢失")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_fraction_offset_and_colons() {
        assert_eq!(
            sanitize_ts_for_filename("2026-07-23T02:48:07.123456789+00:00"),
            "2026-07-23T02-48-07"
        );
    }

    #[test]
    fn sanitize_handles_no_fractional_seconds() {
        assert_eq!(sanitize_ts_for_filename("2026-07-23T02:48:07+00:00"), "2026-07-23T02-48-07");
    }

    #[test]
    fn save_then_read_round_trips_and_names_file_from_doc_and_ts() {
        let tmp = tempfile::tempdir().unwrap();
        let snap = save_snapshot(
            tmp.path(),
            "pi",
            "en",
            "sessions",
            "2026-07-23T02:48:07.1+00:00",
            "old content",
        )
        .unwrap();
        assert_eq!(snap.lang, "en");
        assert_eq!(snap.file, "sessions.2026-07-23T02-48-07.md");
        assert!(snapshot_dir(tmp.path(), "pi", "en").join(&snap.file).exists());
        assert_eq!(read_snapshot(tmp.path(), "pi", &snap).unwrap(), "old content");
    }

    #[test]
    fn save_snapshot_allows_empty_content() {
        let tmp = tempfile::tempdir().unwrap();
        let snap =
            save_snapshot(tmp.path(), "pi", "zh", "intro", "2026-07-23T02:48:07+00:00", "").unwrap();
        assert_eq!(read_snapshot(tmp.path(), "pi", &snap).unwrap(), "");
    }

    #[test]
    fn read_missing_snapshot_errors_with_a_hint() {
        let tmp = tempfile::tempdir().unwrap();
        let snap = SnapshotRef { lang: "en".to_string(), file: "nope.md".to_string() };
        let err = read_snapshot(tmp.path(), "pi", &snap).unwrap_err();
        assert!(err.to_string().contains("不在本机"));
    }
}
