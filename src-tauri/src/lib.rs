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

    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let state = AppState::new(&data_dir)?;
            app.manage(state);
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "Prune started");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_get_meta,
            commands::system_get_info,
            commands::system_get_snapshot,
            commands::system_list_processes,
            commands::cleaner_list_providers,
            commands::cleaner_start_scan,
            commands::cleaner_cancel_scan,
            commands::cleaner_get_scan,
            commands::cleaner_preview,
            commands::cleaner_execute,
            commands::ops_list,
            commands::fs_reveal,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Prune");
}
