//! Tauri shell around `prune-core`. This crate only adapts core APIs to IPC commands and
//! events; it contains no filesystem or OS logic of its own.

mod commands;
mod error;
mod state;

use tauri::Manager;

pub use error::CommandError;
pub use state::AppState;

/// Event names emitted to the frontend.
pub mod events {
    pub const SCAN_PROGRESS: &str = "prune://scan-progress";
    pub const SCAN_COMPLETED: &str = "prune://scan-completed";
    pub const CLEANUP_PROGRESS: &str = "prune://cleanup-progress";
    pub const DISK_PROGRESS: &str = "prune://disk-progress";
    pub const DISK_COMPLETED: &str = "prune://disk-completed";
    pub const APPS_PROGRESS: &str = "prune://apps-progress";
    pub const APPS_COMPLETED: &str = "prune://apps-completed";
}

/// Registers every IPC command on a builder.
///
/// Split out of [`run`] so the integration tests can build the same command surface on
/// Tauri's mock runtime and exercise the real IPC path.
pub fn register_commands<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![
        commands::app_get_meta,
        commands::app_get_permissions,
        commands::app_open_privacy_settings,
        commands::system_get_info,
        commands::system_get_snapshot,
        commands::system_list_processes,
        commands::cleaner_list_providers,
        commands::cleaner_start_scan,
        commands::cleaner_cancel_scan,
        commands::cleaner_get_scan,
        commands::cleaner_preview,
        commands::cleaner_execute,
        commands::disk_start_scan,
        commands::disk_cancel_scan,
        commands::disk_get_summary,
        commands::disk_get_node,
        commands::disk_large_files,
        commands::apps_start_scan,
        commands::apps_get_detail,
        commands::apps_run_uninstaller,
        commands::settings_get,
        commands::settings_set_project_roots,
        commands::startup_list,
        commands::startup_set_enabled,
        commands::ops_list,
        commands::fs_reveal,
    ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("info,prune=debug,prune_core=debug")
            }),
        )
        .with_target(false)
        .init();

    register_commands(tauri::Builder::default())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let state = AppState::new(&data_dir)?;
            app.manage(state);
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "Prune started");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Prune");
}
