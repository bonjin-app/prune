use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};

use prune_core::analyzer::DiskAnalysis;
use prune_core::models::{CleanupPlan, ScanSession};
use prune_core::ops::OperationLog;
use prune_core::platform::{ApplicationInfo, StartupItem};
use prune_core::settings::Settings;
use prune_core::system::SystemMonitor;
use prune_core::PruneEngine;

use crate::recent::Recent;

/// How many finished scans, plans and analyses to keep. Enough that the view the user is
/// looking at and the one before it are always there; small enough that a long session does
/// not hold every directory tree it ever walked.
const RECENT_SCANS: usize = 4;
const RECENT_DISKS: usize = 3;
const RECENT_PLANS: usize = 16;

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
    /// Rebuilt when settings change, so every later command sees the new project roots.
    engine: RwLock<Arc<PruneEngine>>,
    pub settings: Mutex<Settings>,
    pub data_dir: PathBuf,
    pub monitor: Arc<SystemMonitor>,
    pub ops: Arc<OperationLog>,
    pub scans: Mutex<Recent<ScanEntry>>,
    pub plans: Mutex<Recent<CleanupPlan>>,
    pub disks: Mutex<Recent<DiskEntry>>,
    pub apps: Mutex<AppsState>,
    pub startup: Mutex<Vec<StartupItem>>,
}

impl AppState {
    pub fn new(data_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let settings = Settings::load(data_dir);
        let engine = PruneEngine::with_settings(&settings);
        let monitor = SystemMonitor::new(&engine.known_paths().home);
        let ops = OperationLog::open(data_dir)?;
        Ok(Self {
            engine: RwLock::new(Arc::new(engine)),
            settings: Mutex::new(settings),
            data_dir: data_dir.to_path_buf(),
            monitor: Arc::new(monitor),
            ops: Arc::new(ops),
            scans: Mutex::new(Recent::new(RECENT_SCANS)),
            plans: Mutex::new(Recent::new(RECENT_PLANS)),
            disks: Mutex::new(Recent::new(RECENT_DISKS)),
            apps: Mutex::new(AppsState::default()),
            startup: Mutex::new(Vec::new()),
        })
    }

    /// The current engine. Held by `Arc` so a long scan keeps working on the engine it started
    /// with even if settings change underneath it.
    pub fn engine(&self) -> Arc<PruneEngine> {
        self.engine.read().unwrap().clone()
    }

    /// Persists new settings and rebuilds the engine around them.
    pub fn apply_settings(&self, settings: Settings) -> prune_core::Result<()> {
        settings.save(&self.data_dir)?;
        let engine = Arc::new(PruneEngine::with_settings(&settings));
        *self.engine.write().unwrap() = engine;
        *self.settings.lock().unwrap() = settings;
        Ok(())
    }
}
