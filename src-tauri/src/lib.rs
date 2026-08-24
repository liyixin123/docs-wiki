mod backup;
mod commands;
mod config;
mod diff;
mod docs;
mod hashing;
mod local_import;
mod nav;
mod provider;
mod remote_import;
mod search;
mod snapshot;
mod sources;
mod state;
mod sync;
mod translate;

use tauri::Manager;

use state::{load_state, AppState, AppStateData};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app
                .path()
                .app_data_dir()
                .expect("resolving app data dir");
            std::fs::create_dir_all(&dir).expect("creating app data dir");

            let mut data = load_state(&dir).expect("loading state.json").unwrap_or_default();
            bootstrap_sources(&dir, &mut data);

            app.manage(AppState {
                dir,
                data: std::sync::Mutex::new(data),
            });

            // Best-effort background update check, if the user enabled it
            // in settings. Runs after startup, never blocks it.
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = app_handle.state::<AppState>();
                let mut data_snapshot = { state.data.lock().expect("state mutex poisoned").clone() };
                sync::run_startup_checks(&state.dir, &mut data_snapshot).await;
                let mut data = state.data.lock().expect("state mutex poisoned");
                *data = data_snapshot;
                if let Err(e) = state::save_state_atomic(&state.dir, &data) {
                    eprintln!("startup check: failed to save state: {e}");
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_sources,
            commands::list_docs,
            commands::get_nav,
            commands::get_doc_content,
            commands::get_asset_data,
            commands::enable_translation,
            commands::search_docs,
            commands::add_local_source,
            commands::add_remote_source,
            commands::remove_source,
            commands::set_nav_override,
            commands::check_updates,
            commands::apply_update,
            commands::get_history,
            commands::get_config,
            commands::save_config,
            commands::test_provider_connection,
            commands::translate_doc,
            commands::translate_all_pending,
            commands::export_backup,
            commands::import_backup,
            commands::export_library,
            commands::import_library,
            commands::get_diff,
            commands::get_last_reading,
            commands::set_last_reading,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Register any bundled sources that aren't in state.json yet (currently:
/// the Pi seed docs), persisting immediately if anything changed.
fn bootstrap_sources(dir: &std::path::Path, data: &mut AppStateData) {
    match sources::ensure_seed_source(dir, data) {
        Ok(true) => {
            if let Err(e) = state::save_state_atomic(dir, data) {
                eprintln!("failed to persist state after seed import: {e}");
            }
        }
        Ok(false) => {}
        Err(e) => eprintln!("failed to import seed source: {e}"),
    }
}
