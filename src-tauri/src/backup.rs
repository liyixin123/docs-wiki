//! Exporting/importing a portable backup of app *metadata* (`state.json` +
//! `config.json`) — not the actual document content, which lives under
//! `sources/<id>/docs/` and isn't part of this bundle. Restoring a backup
//! onto a machine without matching `sources/` directories will show
//! sources whose docs can't be read until they're re-imported or re-synced.

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::{self, AppConfig};
use crate::state::{self, AppStateData};

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
}
