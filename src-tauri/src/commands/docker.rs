use prune_core::docker::{self, DockerAction, DockerPruneResult, DockerState, SystemRunner};
use tauri::State;

use crate::error::{CommandError, CommandResult};
use crate::AppState;

/// What Docker is holding. Read-only, and never an error: a machine without Docker simply
/// reports that.
#[tauri::command]
pub async fn docker_status() -> CommandResult<DockerState> {
    tauri::async_runtime::spawn_blocking(|| docker::status(&SystemRunner))
        .await
        .map_err(|e| CommandError::new("join", e.to_string()))
}

/// Asks Docker to reclaim space.
///
/// This is the one cleanup that does not go through the safety pipeline, because the data
/// lives inside Docker rather than in files Prune may touch. Only the two conservative
/// commands are reachable, and neither can remove a volume.
#[tauri::command]
pub async fn docker_prune(
    state: State<'_, AppState>,
    action: DockerAction,
) -> CommandResult<DockerPruneResult> {
    let result = tauri::async_runtime::spawn_blocking(move || docker::prune(&SystemRunner, action))
        .await
        .map_err(|e| CommandError::new("join", e.to_string()))??;

    // Docker cleanups belong in the same history as every other cleanup.
    let record = prune_core::models::OperationRecord {
        id: uuid::Uuid::new_v4().to_string(),
        at: chrono::Utc::now(),
        title: format!("Docker: {}", action.label().to_lowercase()),
        mode: prune_core::models::DeleteMode::Permanent,
        status: prune_core::models::CleanupStatus::Success,
        removed_targets: 1,
        removed_files: 0,
        removed_bytes: result.reclaimed_bytes,
        failed_count: 0,
        providers: vec!["docker".into()],
    };
    if let Err(e) = state.ops.append(&record) {
        tracing::warn!(error = %e, "could not record the Docker cleanup");
    }
    // Docker frees real disk space, so the cached free-space reading is now wrong.
    state.monitor.invalidate_disks();
    Ok(result)
}
