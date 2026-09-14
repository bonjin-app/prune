use prune_core::models::OperationRecord;
use tauri::State;

use crate::error::CommandResult;
use crate::AppState;

#[tauri::command]
pub fn ops_list(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> CommandResult<Vec<OperationRecord>> {
    Ok(state.ops.list(limit.unwrap_or(50).min(500))?)
}
