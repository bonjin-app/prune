use prune_core::models::Platform;
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
        platform: state.engine.platform().platform(),
        arch: std::env::consts::ARCH,
        debug: cfg!(debug_assertions),
        operation_log_path: state.ops.path().to_string_lossy().into_owned(),
    })
}
