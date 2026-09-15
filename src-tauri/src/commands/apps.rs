use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use prune_core::apps::{self, AppDetail, AppsProgress};
use prune_core::models::{ScanSession, ScanStatus};
use prune_core::platform::ApplicationInfo;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use crate::error::{CommandError, CommandResult};
use crate::events;
use crate::state::ScanEntry;
use crate::AppState;

/// Lists applications immediately (without sizes) and measures them in the background.
/// Emits `prune://apps-progress` per measured app and `prune://apps-completed` with the full list.
#[tauri::command]
pub fn apps_start_scan<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> CommandResult<Vec<ApplicationInfo>> {
    let list = state.engine().applications()?;
    let scan_id = uuid::Uuid::new_v4().to_string();
    {
        let mut apps_state = state.apps.lock().unwrap();
        apps_state.scan_id = Some(scan_id.clone());
        apps_state.list = list.clone();
    }

    let total = list.len();
    let to_measure = list.clone();
    tauri::async_runtime::spawn_blocking(move || {
        use rayon::prelude::*;
        let cancel = AtomicBool::new(false);
        let done = std::sync::atomic::AtomicUsize::new(0);
        let measured: Vec<ApplicationInfo> = to_measure
            .into_par_iter()
            .map(|mut info| {
                info.size_bytes = apps::measure(&info, &cancel);
                let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                let _ = app.emit(
                    events::APPS_PROGRESS,
                    &AppsProgress {
                        scan_id: scan_id.clone(),
                        done: n,
                        total,
                        app: info.clone(),
                    },
                );
                if let Some(state) = app.try_state::<AppState>() {
                    let mut apps_state = state.apps.lock().unwrap();
                    if apps_state.scan_id.as_deref() == Some(scan_id.as_str()) {
                        if let Some(slot) = apps_state.list.iter_mut().find(|a| a.id == info.id) {
                            slot.size_bytes = info.size_bytes;
                        }
                    }
                }
                info
            })
            .collect();
        let _ = app.emit(events::APPS_COMPLETED, &measured);
    });
    Ok(list)
}

/// Bundle + leftovers for one application. Registers a scan session `app:<id>` so the items
/// can be previewed and removed through `cleaner_preview` / `cleaner_execute`.
#[tauri::command]
pub async fn apps_get_detail(
    state: State<'_, AppState>,
    app_id: String,
) -> CommandResult<AppDetail> {
    let info = state
        .apps
        .lock()
        .unwrap()
        .list
        .iter()
        .find(|a| a.id == app_id)
        .cloned()
        .ok_or_else(|| CommandError::new("unknown_app", app_id.clone()))?;
    let engine = state.engine();
    let detail = tauri::async_runtime::spawn_blocking(move || {
        engine.app_detail(&info, &AtomicBool::new(false))
    })
    .await
    .map_err(|e| CommandError::new("join", e.to_string()))?;

    let session_id = format!("app:{app_id}");
    let mut session = ScanSession {
        id: session_id.clone(),
        status: ScanStatus::Completed,
        started_at: chrono::Utc::now(),
        finished_at: Some(chrono::Utc::now()),
        results: vec![apps::as_scan_result(&detail)],
        total_bytes: 0,
        total_files: 0,
    };
    session.recompute_totals();
    state.scans.lock().unwrap().insert(
        session_id,
        ScanEntry {
            cancel: Arc::new(AtomicBool::new(false)),
            session: Some(session),
        },
    );
    Ok(detail)
}

/// Windows only: launch the vendor's uninstaller (what "Apps & features" does). The user
/// completes the vendor UI; Prune does not run it silently.
#[tauri::command]
pub fn apps_run_uninstaller(state: State<'_, AppState>, app_id: String) -> CommandResult<()> {
    let info = state
        .apps
        .lock()
        .unwrap()
        .list
        .iter()
        .find(|a| a.id == app_id)
        .cloned()
        .ok_or_else(|| CommandError::new("unknown_app", app_id.clone()))?;
    let cmd = info.uninstall_command.ok_or_else(|| {
        CommandError::new("not_implemented", "no uninstaller command for this app")
    })?;
    run_uninstaller(&cmd)
}

#[cfg(target_os = "windows")]
fn run_uninstaller(cmd: &str) -> CommandResult<()> {
    std::process::Command::new("cmd")
        .args(["/C", "start", "", cmd])
        .spawn()?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn run_uninstaller(_cmd: &str) -> CommandResult<()> {
    Err(CommandError::new(
        "not_implemented",
        "vendor uninstallers exist only on Windows",
    ))
}
