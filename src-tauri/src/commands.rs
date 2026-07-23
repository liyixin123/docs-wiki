//! Tauri command surface — the only interface the frontend can call through.
//! Every fallible operation returns `Result<T, String>` since Tauri needs
//! command errors to be serializable; the `String` is a user-facing message.

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::config::{apply_config_input, load_config, save_config_atomic, ConfigInput, PublicConfig};
use crate::docs::read_doc_content;
use crate::local_import::import_local_folder;
use crate::nav::{apply_order_override, NavTree};
use crate::provider::build_provider;
use crate::remote_import::import_remote_source;
use crate::search::{InMemorySearch, SearchBackend, SearchHit};
use crate::state::{now_iso, save_state_atomic, AppState, DocMeta, LogEntry, RemoteSpec, Source, TranslationStatus};
use crate::sources::{read_manifest, remove_source as remove_source_impl};
use crate::sync::{apply_update as apply_update_impl, check_updates as check_updates_impl, CheckSummary};
use crate::translate::translate_doc as translate_doc_impl;

#[tauri::command]
pub fn list_sources(state: State<AppState>) -> Result<Vec<Source>, String> {
    let data = state.data.lock().map_err(|e| e.to_string())?;
    Ok(data.sources.clone())
}

#[tauri::command]
pub fn list_docs(state: State<AppState>, source_id: String) -> Result<Vec<DocMeta>, String> {
    let data = state.data.lock().map_err(|e| e.to_string())?;
    Ok(data
        .docs
        .iter()
        .filter(|d| d.source_id == source_id)
        .cloned()
        .collect())
}

#[tauri::command]
pub fn get_nav(state: State<AppState>, source_id: String) -> Result<NavTree, String> {
    let order_override = {
        let data = state.data.lock().map_err(|e| e.to_string())?;
        data.sources
            .iter()
            .find(|s| s.id == source_id)
            .map(|s| s.order_override.clone())
            .ok_or_else(|| format!("unknown source '{source_id}'"))?
    };

    let mut tree = read_manifest(&state.dir, &source_id).map_err(|e| e.to_string())?;
    apply_order_override(&mut tree.categories, &order_override);
    Ok(tree)
}

#[tauri::command]
pub fn get_doc_content(
    state: State<AppState>,
    source_id: String,
    id: String,
    lang: String,
) -> Result<String, String> {
    read_doc_content(&state.dir, &source_id, &lang, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn search_docs(
    state: State<AppState>,
    query: String,
    source_id: Option<String>,
    lang: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<SearchHit>, String> {
    let data = state.data.lock().map_err(|e| e.to_string())?;
    let index = InMemorySearch::build(&state.dir, &data);
    Ok(index.search(&query, source_id.as_deref(), lang.as_deref(), limit.unwrap_or(20)))
}

#[tauri::command]
pub fn add_local_source(
    state: State<AppState>,
    path: String,
    name: Option<String>,
) -> Result<Source, String> {
    let folder = PathBuf::from(&path);
    let mut data = state.data.lock().map_err(|e| e.to_string())?;
    let source = import_local_folder(&state.dir, &mut data, &folder, name).map_err(|e| e.to_string())?;
    save_state_atomic(&state.dir, &data).map_err(|e| e.to_string())?;
    Ok(source)
}

#[tauri::command]
pub async fn add_remote_source(
    state: State<'_, AppState>,
    owner: String,
    repo: String,
    branch: String,
    path: String,
    name: Option<String>,
    lang: Option<String>,
) -> Result<Source, String> {
    let spec = RemoteSpec { owner, repo, branch, path };
    let lang = lang.unwrap_or_else(|| "en".to_string());

    // The import itself does network I/O, so it can't run while holding a
    // std::sync::MutexGuard across an .await point. Snapshot the state,
    // mutate the snapshot during the (lock-free) fetch, then write it back.
    // Safe in practice: this app only ever has one command running at a
    // time from the UI's perspective (the "import" button disables itself
    // while a request is in flight).
    let mut data_snapshot = { state.data.lock().map_err(|e| e.to_string())?.clone() };
    let source = import_remote_source(&state.dir, &mut data_snapshot, spec, name, lang)
        .await
        .map_err(|e| e.to_string())?;

    {
        let mut data = state.data.lock().map_err(|e| e.to_string())?;
        *data = data_snapshot;
    }
    let data = state.data.lock().map_err(|e| e.to_string())?;
    save_state_atomic(&state.dir, &data).map_err(|e| e.to_string())?;
    Ok(source)
}

#[tauri::command]
pub fn remove_source(state: State<AppState>, source_id: String) -> Result<(), String> {
    let mut data = state.data.lock().map_err(|e| e.to_string())?;
    remove_source_impl(&state.dir, &mut data, &source_id).map_err(|e| e.to_string())?;
    save_state_atomic(&state.dir, &data).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn set_nav_override(
    state: State<AppState>,
    source_id: String,
    ordered_ids: Vec<String>,
) -> Result<(), String> {
    let mut data = state.data.lock().map_err(|e| e.to_string())?;
    let source = data
        .sources
        .iter_mut()
        .find(|s| s.id == source_id)
        .ok_or_else(|| format!("未知的来源 '{source_id}'"))?;
    source.order_override = ordered_ids;
    source.updated_at = now_iso();
    save_state_atomic(&state.dir, &data).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn check_updates(state: State<'_, AppState>, source_id: String) -> Result<CheckSummary, String> {
    // Same snapshot-then-merge pattern as add_remote_source (see its
    // comment): the check does network/disk I/O across many docs, which
    // can't happen while holding a std::sync::MutexGuard across .await.
    let mut data_snapshot = { state.data.lock().map_err(|e| e.to_string())?.clone() };
    let summary = check_updates_impl(&mut data_snapshot, &source_id).await.map_err(|e| e.to_string())?;

    {
        let mut data = state.data.lock().map_err(|e| e.to_string())?;
        *data = data_snapshot;
    }
    let data = state.data.lock().map_err(|e| e.to_string())?;
    save_state_atomic(&state.dir, &data).map_err(|e| e.to_string())?;
    Ok(summary)
}

#[tauri::command]
pub async fn apply_update(state: State<'_, AppState>, source_id: String, id: String) -> Result<DocMeta, String> {
    let mut data_snapshot = { state.data.lock().map_err(|e| e.to_string())?.clone() };
    let doc = apply_update_impl(&state.dir, &mut data_snapshot, &source_id, &id).await.map_err(|e| e.to_string())?;

    {
        let mut data = state.data.lock().map_err(|e| e.to_string())?;
        *data = data_snapshot;
    }
    let data = state.data.lock().map_err(|e| e.to_string())?;
    save_state_atomic(&state.dir, &data).map_err(|e| e.to_string())?;
    Ok(doc)
}

#[tauri::command]
pub fn get_history(
    state: State<AppState>,
    source_id: Option<String>,
    doc_id: Option<String>,
) -> Result<Vec<LogEntry>, String> {
    let data = state.data.lock().map_err(|e| e.to_string())?;
    let mut entries: Vec<LogEntry> = data
        .log
        .iter()
        .filter(|l| source_id.as_deref().is_none_or(|sid| l.source_id == sid))
        .filter(|l| doc_id.as_deref().is_none_or(|did| l.doc_id.as_deref() == Some(did)))
        .cloned()
        .collect();
    entries.reverse(); // most recent first
    Ok(entries)
}

#[tauri::command]
pub fn get_config(state: State<AppState>) -> Result<PublicConfig, String> {
    let cfg = load_config(&state.dir).map_err(|e| e.to_string())?;
    Ok(PublicConfig::from(&cfg))
}

#[tauri::command]
pub fn save_config(state: State<AppState>, input: ConfigInput) -> Result<(), String> {
    let existing = load_config(&state.dir).map_err(|e| e.to_string())?;
    let merged = apply_config_input(&existing, input);
    save_config_atomic(&state.dir, &merged).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn test_provider_connection(state: State<'_, AppState>, provider_id: String) -> Result<(), String> {
    let cfg = load_config(&state.dir).map_err(|e| e.to_string())?;
    let provider_cfg = cfg.providers.get(&provider_id).ok_or_else(|| format!("未知的 provider '{provider_id}'"))?;
    let provider = build_provider(provider_cfg).map_err(|e| e.to_string())?;
    provider.test_connection().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn translate_doc(state: State<'_, AppState>, source_id: String, id: String) -> Result<DocMeta, String> {
    // Same snapshot-then-merge pattern as add_remote_source/check_updates:
    // translation does network I/O, which can't happen while holding a
    // std::sync::MutexGuard across .await.
    let mut data_snapshot = { state.data.lock().map_err(|e| e.to_string())?.clone() };
    let doc = translate_doc_impl(&state.dir, &mut data_snapshot, &source_id, &id).await.map_err(|e| e.to_string())?;

    {
        let mut data = state.data.lock().map_err(|e| e.to_string())?;
        *data = data_snapshot;
    }
    let data = state.data.lock().map_err(|e| e.to_string())?;
    save_state_atomic(&state.dir, &data).map_err(|e| e.to_string())?;
    Ok(doc)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TranslateProgress {
    done: usize,
    total: usize,
    current_title: String,
    success: bool,
    error: Option<String>,
}

#[tauri::command]
pub async fn translate_all_pending(
    app: AppHandle,
    state: State<'_, AppState>,
    source_id: String,
) -> Result<(), String> {
    let pending: Vec<(String, String)> = {
        let data = state.data.lock().map_err(|e| e.to_string())?;
        data.docs
            .iter()
            .filter(|d| {
                d.source_id == source_id
                    && matches!(d.translation_status, TranslationStatus::Pending | TranslationStatus::NeverTranslated)
            })
            .map(|d| (d.id.clone(), d.title.clone()))
            .collect()
    };

    let total = pending.len();
    for (index, (doc_id, title)) in pending.into_iter().enumerate() {
        let mut data_snapshot = { state.data.lock().map_err(|e| e.to_string())?.clone() };
        let result = translate_doc_impl(&state.dir, &mut data_snapshot, &source_id, &doc_id).await;

        let success = result.is_ok();
        let error = result.err().map(|e| e.to_string());
        if success {
            let mut data = state.data.lock().map_err(|e| e.to_string())?;
            *data = data_snapshot;
            let data = state.data.lock().map_err(|e| e.to_string())?;
            save_state_atomic(&state.dir, &data).map_err(|e| e.to_string())?;
        }

        // Best-effort: a batch translation shouldn't abort just because the
        // event channel hiccuped — the frontend can still poll list_docs.
        let _ = app.emit(
            "translate-progress",
            TranslateProgress { done: index + 1, total, current_title: title, success, error },
        );
    }
    Ok(())
}

#[tauri::command]
pub fn export_backup(state: State<AppState>, dest_path: String) -> Result<(), String> {
    crate::backup::export_backup(&state.dir, &PathBuf::from(dest_path)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn import_backup(state: State<AppState>, src_path: String) -> Result<(), String> {
    let restored = crate::backup::import_backup(&state.dir, &PathBuf::from(src_path)).map_err(|e| e.to_string())?;
    let mut data = state.data.lock().map_err(|e| e.to_string())?;
    *data = restored;
    Ok(())
}
