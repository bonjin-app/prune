use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use prune_core::analyzer::DiskAnalysis;
use prune_core::models::{CleanupPlan, ScanSession};
use prune_core::ops::OperationLog;
use prune_core::platform::ApplicationInfo;
use prune_core::system::SystemMonitor;
use prune_core::PruneEngine;

/// Bookkeeping for a scan that is running or finished.
pub struct ScanEntry {
    pub cancel: Arc<AtomicBool>,
    pub session: Option<ScanSession>,
}

/// Bookkeeping for a disk analysis.
pub struct DiskEntry {
    pub cancel: Arc<AtomicBool>,
    pub root: PathBuf,
    pub analysis: Option<DiskAnalysis>,
}

/// Cached application list (Uninstaller).
#[derive(Default)]
pub struct AppsState {
    pub scan_id: Option<String>,
    pub list: Vec<ApplicationInfo>,
}

/// Process-wide state managed by Tauri.
pub struct AppState {
    pub engine: Arc<PruneEngine>,
    pub monitor: Arc<SystemMonitor>,
    pub ops: Arc<OperationLog>,
    pub scans: Mutex<HashMap<String, ScanEntry>>,
    pub plans: Mutex<HashMap<String, CleanupPlan>>,
    pub disks: Mutex<HashMap<String, DiskEntry>>,
    pub apps: Mutex<AppsState>,
}

impl AppState {
    pub fn new(data_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let engine = PruneEngine::new();
        let monitor = SystemMonitor::new(&engine.known_paths().home);
        let ops = OperationLog::open(data_dir)?;
        Ok(Self {
            engine: Arc::new(engine),
            monitor: Arc::new(monitor),
            ops: Arc::new(ops),
            scans: Mutex::new(HashMap::new()),
            plans: Mutex::new(HashMap::new()),
            disks: Mutex::new(HashMap::new()),
            apps: Mutex::new(AppsState::default()),
        })
    }
}
