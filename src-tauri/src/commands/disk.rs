use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use prune_core::analyzer::{
    self, DiskAnalysis, DiskNodeView, DiskProgress, DiskScanStatus, DiskSummary, LargeFile,
};
use prune_core::models::{ScanSession, ScanStatus};
use prune_core::sync::LockExt;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use crate::error::{CommandError, CommandResult};
use crate::events;
use crate::state::{DiskEntry, ScanEntry};
use crate::AppState;

/// Keep partial results available to the existing preview pipeline while retaining the
/// analysis lifecycle, so a cancelled scan is never presented as completed.
fn large_files_session(analysis: &DiskAnalysis) -> ScanSession {
    let mut session = ScanSession {
        id: analysis.scan_id.clone(),
        status: match analysis.status {
            DiskScanStatus::Running => ScanStatus::Running,
            DiskScanStatus::Completed => ScanStatus::Completed,
            DiskScanStatus::Cancelled => ScanStatus::Cancelled,
        },
        started_at: analysis.started_at,
        finished_at: Some(chrono::Utc::now()),
        results: vec![analysis.large_files_result()],
        total_bytes: 0,
        total_files: 0,
    };
    session.recompute_totals();
    session
}

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
        _ => state.engine().known_paths().home.clone(),
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
    state.disks.lock_recover().insert(
        scan_id.clone(),
        DiskEntry {
            cancel: cancel.clone(),
            analysis: None,
            root: root.clone(),
        },
    );

    let engine = state.engine();
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
            let session = large_files_session(&analysis);
            state.scans.lock_recover().insert(
                id.clone(),
                ScanEntry {
                    cancel: Arc::new(AtomicBool::new(false)),
                    session: Some(session),
                },
            );
            if let Some(entry) = state.disks.lock_recover().get_mut(&id) {
                entry.analysis = Some(analysis);
            }
        }
        let _ = app.emit(events::DISK_COMPLETED, &summary);
    });
    Ok(scan_id)
}

#[tauri::command]
pub fn disk_cancel_scan(state: State<'_, AppState>, scan_id: String) -> CommandResult<()> {
    let disks = state.disks.lock_recover();
    let entry = disks
        .get(&scan_id)
        .ok_or_else(|| CommandError::new("unknown_scan", scan_id.clone()))?;
    entry.cancel.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub fn disk_get_summary(state: State<'_, AppState>, scan_id: String) -> CommandResult<DiskSummary> {
    let disks = state.disks.lock_recover();
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
    let disks = state.disks.lock_recover();
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
    let disks = state.disks.lock_recover();
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

#[cfg(test)]
mod tests {
    use super::*;
    use prune_core::{platform::ProtectedPaths, safety::SafetyPolicy};

    #[test]
    fn large_files_session_preserves_status_and_discovered_targets() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("file.bin"), [0; 17]).unwrap();
        let policy = SafetyPolicy::from_protected(ProtectedPaths::default());
        let mut analysis = analyzer::analyze(
            "disk-session",
            dir.path(),
            &policy,
            &AtomicBool::new(false),
            &|_| {},
        );
        let completed = large_files_session(&analysis);
        assert_eq!(completed.status, ScanStatus::Completed);

        // Model a scan cancelled after discovering a file, without a timing-sensitive race.
        analysis.status = DiskScanStatus::Cancelled;
        let cancelled = large_files_session(&analysis);
        assert_eq!(cancelled.status, ScanStatus::Cancelled);
        assert_eq!(cancelled.id, analysis.scan_id);
        assert_eq!(cancelled.started_at, analysis.started_at);
        assert!(cancelled.finished_at.is_some());
        assert_eq!(cancelled.total_bytes, 17);
        assert_eq!(cancelled.total_files, 1);
        assert_eq!(cancelled.results[0].targets.len(), 1);
        assert_eq!(
            cancelled.results[0].targets[0].id,
            completed.results[0].targets[0].id
        );
    }
}
