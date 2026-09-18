use prune_core::platform::StartupItem;
use prune_core::sync::LockExt;
use tauri::State;

use crate::error::{CommandError, CommandResult};
use crate::AppState;

/// Programs that start by themselves. Read-only.
#[tauri::command]
pub async fn startup_list(state: State<'_, AppState>) -> CommandResult<Vec<StartupItem>> {
    let engine = state.engine();
    let items = tauri::async_runtime::spawn_blocking(move || engine.startup_items())
        .await
        .map_err(|e| CommandError::new("join", e.to_string()))??;
    *state.startup.lock_recover() = items.clone();
    Ok(items)
}

/// Enable or disable one item. Nothing is deleted and the change is reversible.
#[tauri::command]
pub async fn startup_set_enabled(
    state: State<'_, AppState>,
    item_id: String,
    enabled: bool,
) -> CommandResult<StartupItem> {
    let item = state
        .startup
        .lock()
        .unwrap()
        .iter()
        .find(|i| i.id == item_id)
        .cloned()
        .ok_or_else(|| CommandError::new("unknown_startup_item", item_id.clone()))?;
    let engine = state.engine();
    let target = item.clone();
    tauri::async_runtime::spawn_blocking(move || engine.set_startup_enabled(&target, enabled))
        .await
        .map_err(|e| CommandError::new("join", e.to_string()))??;

    let updated = StartupItem { enabled, ..item };
    if let Some(slot) = state
        .startup
        .lock()
        .unwrap()
        .iter_mut()
        .find(|i| i.id == item_id)
    {
        slot.enabled = enabled;
    }
    Ok(updated)
}
