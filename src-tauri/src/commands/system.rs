use prune_core::models::{ProcessInfo, StopMode, SystemInfo, SystemSnapshot};
use tauri::State;

use crate::error::{CommandError, CommandResult};
use crate::AppState;

fn join_err(e: impl std::fmt::Display) -> CommandError {
    CommandError::new("join", e.to_string())
}

#[tauri::command]
pub async fn system_get_info(state: State<'_, AppState>) -> CommandResult<SystemInfo> {
    let monitor = state.monitor.clone();
    tauri::async_runtime::spawn_blocking(move || monitor.info())
        .await
        .map_err(join_err)
}

#[tauri::command]
pub async fn system_get_snapshot(state: State<'_, AppState>) -> CommandResult<SystemSnapshot> {
    let monitor = state.monitor.clone();
    tauri::async_runtime::spawn_blocking(move || monitor.snapshot())
        .await
        .map_err(join_err)
}

#[tauri::command]
pub async fn system_list_processes(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> CommandResult<Vec<ProcessInfo>> {
    let monitor = state.monitor.clone();
    let limit = limit.unwrap_or(50).min(500);
    tauri::async_runtime::spawn_blocking(move || monitor.processes(limit))
        .await
        .map_err(join_err)
}

/// Asks a process to stop, or forces it.
///
/// The engine re-checks its own protection rules before signalling anything, so a stale process
/// id from the UI cannot reach a process Prune refuses to touch.
///
/// `name` is what the user was shown when they confirmed. The engine checks the id still belongs
/// to it, because a recycled id on an ordinary process passes every protection rule — and then
/// the program that dies is not the one anybody agreed to.
#[tauri::command]
pub async fn system_stop_process(
    state: State<'_, AppState>,
    pid: u32,
    mode: StopMode,
    name: Option<String>,
) -> CommandResult<()> {
    let monitor = state.monitor.clone();
    tauri::async_runtime::spawn_blocking(move || {
        monitor.stop_process_named(pid, mode, name.as_deref())
    })
    .await
    .map_err(join_err)??;
    Ok(())
}
