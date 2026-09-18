use prune_core::models::Platform;
use prune_core::platform::Permissions;
use prune_core::providers::CustomIssue;
use serde::Serialize;
use tauri::State;

use crate::error::CommandResult;
use crate::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppMeta {
    pub name: &'static str,
    pub version: &'static str,
    pub core_version: &'static str,
    pub platform: Platform,
    pub arch: &'static str,
    pub debug: bool,
    pub operation_log_path: String,
}

#[tauri::command]
pub fn app_get_meta(state: State<'_, AppState>) -> CommandResult<AppMeta> {
    Ok(AppMeta {
        name: "Prune",
        version: env!("CARGO_PKG_VERSION"),
        core_version: prune_core::VERSION,
        platform: state.engine().platform().platform(),
        arch: std::env::consts::ARCH,
        debug: cfg!(debug_assertions),
        operation_log_path: state.ops.path().to_string_lossy().into_owned(),
    })
}

/// What Prune may read on this machine. Cheap enough to call on every view.
#[tauri::command]
pub fn app_get_permissions(state: State<'_, AppState>) -> CommandResult<Permissions> {
    Ok(state.engine().permissions())
}

/// Opens the OS privacy settings so the user can grant the missing permission.
#[tauri::command]
pub fn app_open_privacy_settings(state: State<'_, AppState>) -> CommandResult<()> {
    state.engine().open_privacy_settings()?;
    Ok(())
}

/// Anything wrong with the user's `providers.json`.
///
/// Shown rather than swallowed: a provider that failed to load looks exactly like one that
/// found nothing, and a mistake in the file would read as "that cache is already clean".
#[tauri::command]
pub fn providers_custom_issues(state: State<'_, AppState>) -> CommandResult<Vec<CustomIssue>> {
    Ok(state.engine().custom_provider_issues().to_vec())
}
