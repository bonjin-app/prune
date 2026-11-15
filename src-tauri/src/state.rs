use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};

use prune_core::analyzer::DiskAnalysis;
use prune_core::models::{CleanupPlan, ScanSession};
use prune_core::ops::OperationLog;
use prune_core::platform::{ApplicationInfo, StartupItem};
use prune_core::settings::Settings;
use prune_core::sync::{LockExt, RwLockExt};
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
    /// Why the settings file could not be read, while that is still the case. Every removal is
    /// refused meanwhile, because what the user protected is unknown.
    pub settings_problem: Mutex<Option<String>>,
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
    /// Never fails: an unusable data directory costs the user their settings and history, not
    /// the application. A bundled app that refuses to start has no console to explain itself.
    pub fn new(data_dir: &Path) -> Self {
        let loaded = Settings::load_checked(data_dir);
        let settings = loaded.settings;
        let mut engine = PruneEngine::configured(&settings, data_dir);
        if let Some(problem) = &loaded.problem {
            engine.refuse_all_removal(problem.clone());
        }
        let monitor = SystemMonitor::new(&engine.known_paths().home);
        let ops = OperationLog::open(data_dir);
        Self {
            engine: RwLock::new(Arc::new(engine)),
            settings: Mutex::new(settings),
            settings_problem: Mutex::new(loaded.problem),
            data_dir: data_dir.to_path_buf(),
            monitor: Arc::new(monitor),
            ops: Arc::new(ops),
            scans: Mutex::new(Recent::new(RECENT_SCANS)),
            plans: Mutex::new(Recent::new(RECENT_PLANS)),
            disks: Mutex::new(Recent::new(RECENT_DISKS)),
            apps: Mutex::new(AppsState::default()),
            startup: Mutex::new(Vec::new()),
        }
    }

    /// The current engine. Held by `Arc` so a long scan keeps working on the engine it started
    /// with even if settings change underneath it.
    pub fn engine(&self) -> Arc<PruneEngine> {
        self.engine.read_recover().clone()
    }

    /// Persists new settings and rebuilds the engine around them.
    ///
    /// If the file on disk could not be read, it is moved aside first rather than overwritten:
    /// the user may still want what was in it, and the settings they save now become the ones
    /// that count.
    pub fn apply_settings(&self, settings: Settings) -> prune_core::Result<()> {
        let mut problem = self.settings_problem.lock_recover();
        if problem.is_some() {
            if let Some(aside) = Settings::set_aside_unreadable(&self.data_dir)? {
                tracing::warn!(path = %aside.display(), "kept the unreadable settings file");
            }
        }
        settings.save(&self.data_dir)?;
        *problem = None;
        let engine = Arc::new(PruneEngine::configured(&settings, &self.data_dir));
        *self.engine.write_recover() = engine;
        *self.settings.lock_recover() = settings;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unreadable_settings_refuse_removal_until_new_ones_are_saved_and_keep_the_old_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("settings.json"), b"{ \"protectedPaths\": [").unwrap();
        let state = AppState::new(dir.path());
        assert!(state.settings_problem.lock_recover().is_some());

        // Something Prune would otherwise remove: inside the temporary folder it may work in.
        let victim = tempfile::Builder::new()
            .prefix("prune-state-test")
            .tempdir_in(std::env::temp_dir())
            .unwrap();
        let refused = state.engine().policy().validate(victim.path());
        assert!(
            refused
                .as_ref()
                .is_err_and(|e| e.to_string().contains("could not be read")),
            "{refused:?}"
        );

        state.apply_settings(Settings::default()).unwrap();
        assert!(state.settings_problem.lock_recover().is_none());
        assert!(state.engine().policy().validate(victim.path()).is_ok());
        let kept: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("settings.unreadable-"))
            .collect();
        assert_eq!(
            kept.len(),
            1,
            "the unreadable file is kept beside the new one"
        );
    }
}
