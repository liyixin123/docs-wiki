//! Core data model shared across the app: sources, documents, and the log,
//! plus atomic (write-temp-then-rename) persistence to `state.json`.

use std::fs;
use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// How a source's content got onto disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceKind {
    /// Bundled with the app (e.g. the built-in Pi docs).
    Seed,
    /// Imported from a local folder the user picked.
    LocalFolder,
    /// Fetched from a remote git host (currently: GitHub raw URLs).
    RemoteGit,
}

/// Where to fetch upstream updates from, for sources that support sync.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSpec {
    pub owner: String,
    pub repo: String,
    pub branch: String,
    /// Subdirectory inside the repo that holds the docs (may be empty).
    pub path: String,
}

/// A registered documentation collection (one Pi docs set, one imported
/// folder, one remote wiki, etc). Multiple sources can coexist.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub id: String,
    pub name: String,
    pub kind: SourceKind,
    /// e.g. ["en", "zh"] for a bilingual source, or ["en"] for a single-language one.
    pub languages: Vec<String>,
    pub primary_language: String,
    pub remote: Option<RemoteSpec>,
    /// Absolute path this source was imported from, for `LocalFolder` sources.
    /// Needed to re-scan for local changes later (P4); `None` for other kinds.
    #[serde(default)]
    pub local_path: Option<String>,
    /// User-defined manual ordering of doc ids; empty means "use the parsed nav order".
    #[serde(default)]
    pub order_override: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CheckStatus {
    Same,
    Changed,
    Error,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TranslationStatus {
    Translated,
    Pending,
    NeverTranslated,
    /// Translated, but the post-translation structural self-check (heading
    /// / code-fence / table counts) didn't match the original — flagged for
    /// a human to look at rather than silently marked done.
    NeedsReview,
    /// Source is single-language; translation concepts do not apply.
    NotApplicable,
}

/// Metadata for a single document within a source (language-independent;
/// each language variant lives at `sources/<source_id>/docs/<lang>/<id>.md`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocMeta {
    pub source_id: String,
    pub id: String,
    pub title: String,
    pub category: String,
    /// sha256 of the primary-language content, used to detect upstream changes.
    pub primary_hash: String,
    pub last_checked_at: Option<String>,
    pub last_check_status: CheckStatus,
    pub translation_status: TranslationStatus,
    pub translated_at: Option<String>,
    pub translated_by: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LogKind {
    Import,
    Check,
    ApplyUpdate,
    Translate,
    TranslateError,
}

/// Points at a saved snapshot of a doc's content from *before* an
/// `apply_update` (primary-language file) or `translate` (target-language
/// file) overwrite, so the history UI can diff the old vs. current content.
/// `file` is the snapshot's file name, relative to
/// `sources/<id>/.history/<lang>/`. `None` for entries that don't describe an
/// in-place content change (imports, update checks), and for first-time
/// apply/translate where there was no prior file to snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRef {
    pub lang: String,
    pub file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub id: String,
    pub ts: String,
    pub source_id: String,
    pub doc_id: Option<String>,
    pub kind: LogKind,
    pub detail: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<SnapshotRef>,
}

/// The reading position to restore on next launch: what the user had open
/// when the app last closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastReading {
    pub source_id: String,
    pub doc_id: String,
    pub lang: String,
}

/// The full persisted app state (serialized as `state.json`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStateData {
    #[serde(default)]
    pub sources: Vec<Source>,
    #[serde(default)]
    pub docs: Vec<DocMeta>,
    #[serde(default)]
    pub log: Vec<LogEntry>,
    /// Last doc the user was reading; restored on next startup.
    #[serde(default)]
    pub last_reading: Option<LastReading>,
}

/// Tauri-managed application state: the resolved app data directory plus
/// the in-memory copy of `state.json`, guarded by a mutex.
pub struct AppState {
    pub dir: std::path::PathBuf,
    pub data: Mutex<AppStateData>,
}

impl AppState {
    // Unused until P3+ commands (add_local_source, add_remote_source, apply_update, ...)
    // start mutating `data` at runtime and need to persist the result.
    #[allow(dead_code)]
    pub fn state_file(&self) -> std::path::PathBuf {
        self.dir.join("state.json")
    }

    /// Persist the current in-memory state to disk using a temp-file +
    /// rename so a crash mid-write can never leave a corrupt state.json.
    #[allow(dead_code)]
    pub fn save(&self) -> Result<()> {
        let data = self.data.lock().expect("state mutex poisoned");
        save_state_atomic(&self.dir, &data)
    }
}

/// Write `state.json` atomically: write to `state.json.tmp` then rename
/// over the real file. Rename is atomic on the same filesystem, so a crash
/// or power loss mid-write can't corrupt the previous good state.json.
pub fn save_state_atomic(dir: &Path, data: &AppStateData) -> Result<()> {
    fs::create_dir_all(dir).with_context(|| format!("creating state dir {dir:?}"))?;
    let final_path = dir.join("state.json");
    let tmp_path = dir.join("state.json.tmp");
    let json = serde_json::to_string_pretty(data).context("serializing app state")?;
    fs::write(&tmp_path, json).with_context(|| format!("writing {tmp_path:?}"))?;
    fs::rename(&tmp_path, &final_path).with_context(|| format!("renaming into {final_path:?}"))?;
    Ok(())
}

/// Load `state.json` if present. Returns `Ok(None)` on first run.
pub fn load_state(dir: &Path) -> Result<Option<AppStateData>> {
    let path = dir.join("state.json");
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&path).with_context(|| format!("reading {path:?}"))?;
    let data: AppStateData =
        serde_json::from_str(&content).with_context(|| format!("parsing {path:?}"))?;
    Ok(Some(data))
}

/// Current UTC time as an RFC3339 string, used for all timestamps we persist.
pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}
