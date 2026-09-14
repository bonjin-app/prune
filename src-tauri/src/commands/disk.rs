use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use prune_core::analyzer::{self, DiskNodeView, DiskProgress, DiskSummary, LargeFile};
use prune_core::models::{ScanSession, ScanStatus};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use crate::error::{CommandError, CommandResult};
use crate::events;
use crate::state::{DiskEntry, ScanEntry};
use crate::AppState;

/// Starts a read-only disk analysis of `root` (defaults to the home directory).
/// Emits `prune://disk-progress` and `prune://disk-completed` (a `DiskSummary`).
#[tauri::command]
pub fn disk_start_scan<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    root: Option<String>,
) -> CommandResult<String> {
    let root: PathBuf = match root {
        Some(r) if !r.trim().is_empty() => PathBuf::from(r.trim()),
        _ => state.engine.known_paths().home.clone(),
    };
    if !root.is_absolute() {
        return Err(CommandError::new("invalid_path", "path must be absolute"));
    }
    if !root.is_dir() {
        return Err(CommandError::new("invalid_path", "path is not a directory"));
    }
    let root = std::fs::canonicalize(&root).unwrap_or(root);

    let scan_id = uuid::Uuid::new_v4().to_string();
    let cancel = Arc::new(AtomicBool::new(false));
    state.disks.lock().unwrap().insert(
        scan_id.clone(),
        DiskEntry {
            cancel: cancel.clone(),
            analysis: None,
            root: root.clone(),
        },
    );

    let engine = state.engine.clone();
    let id = scan_id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let progress_app = app.clone();
        let on_progress = move |p: DiskProgress| {
            let _ = progress_app.emit(events::DISK_PROGRESS, &p);
        };
        let analysis = analyzer::analyze(&id, &root, engine.policy(), &cancel, &on_progress);
        let summary = analysis.summary();
        if let Some(state) = app.try_state::<AppState>() {
            // Expose the large files as a scan session so the normal preview/execute pipeline
            // (target ids → validated paths) applies to them.
            let mut session = ScanSession {
                id: id.clone(),
                status: ScanStatus::Completed,
                started_at: analysis.started_at,
                finished_at: Some(chrono::Utc::now()),
                results: vec![analysis.large_files_result()],
                total_bytes: 0,
                total_files: 0,
            };
            session.recompute_totals();
            state.scans.lock().unwrap().insert(
                id.clone(),
                ScanEntry {
                    cancel: Arc::new(AtomicBool::new(false)),
                    session: Some(session),
                },
            );
            if let Some(entry) = state.disks.lock().unwrap().get_mut(&id) {
                entry.analysis = Some(analysis);
            }
        }
        let _ = app.emit(events::DISK_COMPLETED, &summary);
    });
    Ok(scan_id)
}

#[tauri::command]
pub fn disk_cancel_scan(state: State<'_, AppState>, scan_id: String) -> CommandResult<()> {
    let disks = state.disks.lock().unwrap();
    let entry = disks
        .get(&scan_id)
        .ok_or_else(|| CommandError::new("unknown_scan", scan_id.clone()))?;
    entry.cancel.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub fn disk_get_summary(state: State<'_, AppState>, scan_id: String) -> CommandResult<DiskSummary> {
    let disks = state.disks.lock().unwrap();
    let entry = disks
        .get(&scan_id)
        .ok_or_else(|| CommandError::new("unknown_scan", scan_id.clone()))?;
    let analysis = entry
        .analysis
        .as_ref()
        .ok_or_else(|| CommandError::new("scan_running", "analysis not finished"))?;
    Ok(analysis.summary())
}

/// Directory node with children. `path` defaults to the scan root.
#[tauri::command]
pub fn disk_get_node(
    state: State<'_, AppState>,
    scan_id: String,
    path: Option<String>,
) -> CommandResult<DiskNodeView> {
    let disks = state.disks.lock().unwrap();
    let entry = disks
        .get(&scan_id)
        .ok_or_else(|| CommandError::new("unknown_scan", scan_id.clone()))?;
    let analysis = entry
        .analysis
        .as_ref()
        .ok_or_else(|| CommandError::new("scan_running", "analysis not finished"))?;
    analysis
        .node(path.as_deref().map(Path::new))
        .ok_or_else(|| CommandError::new("unknown_path", "path was not part of this scan"))
}

#[tauri::command]
pub fn disk_large_files(
    state: State<'_, AppState>,
    scan_id: String,
    min_bytes: Option<u64>,
    limit: Option<usize>,
) -> CommandResult<Vec<LargeFile>> {
    let disks = state.disks.lock().unwrap();
    let entry = disks
        .get(&scan_id)
        .ok_or_else(|| CommandError::new("unknown_scan", scan_id.clone()))?;
    let analysis = entry
        .analysis
        .as_ref()
        .ok_or_else(|| CommandError::new("scan_running", "analysis not finished"))?;
    Ok(analysis.large_files(
        min_bytes.unwrap_or(1_000_000_000),
        limit.unwrap_or(200).min(2000),
    ))
}
