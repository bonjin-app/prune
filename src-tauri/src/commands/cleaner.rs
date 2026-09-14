use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use prune_core::models::{
    CleanupPlan, CleanupResult, DeleteMode, ProviderInfo, ScanProgress, ScanSession, ScanStatus,
};
use prune_core::scan::ScanRequest;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::{CommandError, CommandResult};
use crate::events;
use crate::state::ScanEntry;
use crate::AppState;

#[tauri::command]
pub fn cleaner_list_providers(state: State<'_, AppState>) -> CommandResult<Vec<ProviderInfo>> {
    Ok(state.engine.providers())
}

/// Starts a scan on a worker thread and returns its id immediately.
/// Progress arrives via `prune://scan-progress`; the finished session via `prune://scan-completed`.
#[tauri::command]
pub fn cleaner_start_scan(
    app: AppHandle,
    state: State<'_, AppState>,
    provider_ids: Option<Vec<String>>,
) -> CommandResult<String> {
    let scan_id = uuid::Uuid::new_v4().to_string();
    let cancel = Arc::new(AtomicBool::new(false));
    state.scans.lock().unwrap().insert(
        scan_id.clone(),
        ScanEntry {
            cancel: cancel.clone(),
            session: None,
        },
    );

    let engine = state.engine.clone();
    let request = ScanRequest { provider_ids };
    let id = scan_id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let progress_app = app.clone();
        let on_progress = move |p: ScanProgress| {
            let _ = progress_app.emit(events::SCAN_PROGRESS, &p);
        };
        let session = engine.scan(&id, &request, cancel, &on_progress);
        if let Some(state) = app.try_state::<AppState>() {
            if let Some(entry) = state.scans.lock().unwrap().get_mut(&id) {
                entry.session = Some(session.clone());
            }
        }
        let _ = app.emit(events::SCAN_COMPLETED, &session);
    });

    Ok(scan_id)
}

#[tauri::command]
pub fn cleaner_cancel_scan(state: State<'_, AppState>, scan_id: String) -> CommandResult<()> {
    let scans = state.scans.lock().unwrap();
    let entry = scans
        .get(&scan_id)
        .ok_or_else(|| CommandError::new("unknown_scan", scan_id.clone()))?;
    entry.cancel.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub fn cleaner_get_scan(state: State<'_, AppState>, scan_id: String) -> CommandResult<ScanSession> {
    let scans = state.scans.lock().unwrap();
    let entry = scans
        .get(&scan_id)
        .ok_or_else(|| CommandError::new("unknown_scan", scan_id.clone()))?;
    entry
        .session
        .clone()
        .ok_or_else(|| CommandError::new("scan_running", "scan has not finished yet"))
}

/// Dry run. Nothing is removed.
#[tauri::command]
pub fn cleaner_preview(
    state: State<'_, AppState>,
    scan_id: String,
    target_ids: Vec<String>,
    mode: Option<DeleteMode>,
) -> CommandResult<CleanupPlan> {
    let session = {
        let scans = state.scans.lock().unwrap();
        let entry = scans
            .get(&scan_id)
            .ok_or_else(|| CommandError::new("unknown_scan", scan_id.clone()))?;
        entry
            .session
            .clone()
            .ok_or_else(|| CommandError::new("scan_running", "scan has not finished yet"))?
    };
    if session.status == ScanStatus::Running {
        return Err(CommandError::new(
            "scan_running",
            "scan has not finished yet",
        ));
    }
    let plan = state
        .engine
        .plan(&session, &target_ids, mode.unwrap_or_default());
    state
        .plans
        .lock()
        .unwrap()
        .insert(plan.id.clone(), plan.clone());
    Ok(plan)
}

/// Executes a previously previewed plan. The plan id is the only input: paths never cross IPC
/// in this direction. A plan can be executed once.
#[tauri::command]
pub async fn cleaner_execute(
    app: AppHandle,
    state: State<'_, AppState>,
    plan_id: String,
) -> CommandResult<CleanupResult> {
    let plan = state
        .plans
        .lock()
        .unwrap()
        .remove(&plan_id)
        .ok_or_else(|| CommandError::new("unknown_plan", plan_id.clone()))?;
    let engine = state.engine.clone();
    let ops = state.ops.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let on_progress = |p: prune_core::models::CleanupProgress| {
            let _ = app.emit(events::CLEANUP_PROGRESS, &p);
        };
        engine.execute(&plan, Some(ops.as_ref()), &on_progress)
    })
    .await
    .map_err(|e| CommandError::new("join", e.to_string()))??;

    // Drop the scan the plan came from: its sizes are stale now.
    state.scans.lock().unwrap().retain(|_, e| {
        e.session
            .as_ref()
            .map(|s| s.id != result.plan_id)
            .unwrap_or(true)
    });
    Ok(result)
}
