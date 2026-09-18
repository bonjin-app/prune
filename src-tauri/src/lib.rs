//! Tauri shell around `prune-core`. This crate only adapts core APIs to IPC commands and
//! events; it contains no filesystem or OS logic of its own.

mod commands;
mod error;
mod recent;
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
/// The plugins that make Prune behave like a desktop application rather than a process.
///
/// Neither can see anything of the user's: one remembers where the window was, the other keeps
/// a second copy from scanning the same machine in parallel and disagreeing with the first.
/// Separated from `run` so the tests can build an app with the same plugin stack.
pub fn register_plugins<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.plugin(tauri_plugin_window_state::Builder::default().build())
}

pub fn register_commands<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![
        commands::app_get_meta,
        commands::app_get_permissions,
        commands::providers_custom_issues,
        commands::app_open_privacy_settings,
        commands::system_get_info,
        commands::system_get_snapshot,
        commands::system_list_processes,
        commands::system_stop_process,
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
        commands::docker_status,
        commands::docker_prune,
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

    // Single instance has to be registered first, and only in the real application: it exits
    // the process when another copy is already running, which a test harness must never do.
    let builder =
        tauri::Builder::default().plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }));

    register_commands(register_plugins(builder))
        .setup(|app| {
            // Everything below degrades rather than refuses. A window that never appears,
            // with no console behind it, is the worst failure a bundled app can have.
            let data_dir = app.path().app_data_dir().unwrap_or_else(|e| {
                tracing::warn!(error = %e, "no application data directory; settings and history will not persist");
                std::env::temp_dir().join("app.bonjin.prune")
            });
            app.manage(AppState::new(&data_dir));
            tracing::info!(
                version = env!("CARGO_PKG_VERSION"),
                data_dir = %data_dir.display(),
                "Prune started"
            );
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Prune");
}
