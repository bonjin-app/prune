use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};

use prune_core::analyzer::DiskAnalysis;
use prune_core::models::{CleanupPlan, ScanSession};
use prune_core::ops::OperationLog;
use prune_core::platform::{ApplicationInfo, StartupItem};
use prune_core::settings::{Fingerprint, Settings};
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
    /// Read through [`AppState::current_settings`], which first picks up a file saved by the
    /// command line in the meantime.
    settings: Mutex<Settings>,
    /// The version of the settings file `settings` and the engine were built from.
    loaded_from: Mutex<Option<Fingerprint>>,
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
        let fingerprint = Settings::fingerprint(data_dir);
        let loaded = Settings::load_checked(data_dir);
        let settings = loaded.settings;
        let engine = build_engine(&settings, loaded.problem.as_deref(), data_dir);
        let monitor = SystemMonitor::new(&engine.known_paths().home);
        let ops = OperationLog::open(data_dir);
        Self {
            engine: RwLock::new(Arc::new(engine)),
            settings: Mutex::new(settings),
            loaded_from: Mutex::new(fingerprint),
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
    ///
    /// Picks up a settings file saved elsewhere first: a path protected with `prune protect`
    /// while the app is open must be honoured by the next preview and the next removal, not
    /// after a restart.
    pub fn engine(&self) -> Arc<PruneEngine> {
        self.sync_settings();
        self.engine.read_recover().clone()
    }

    /// The settings as they are now on disk. Changes are made to this, never to an older copy,
    /// so saving from the app cannot undo what the command line saved meanwhile.
    pub fn current_settings(&self) -> Settings {
        self.sync_settings();
        self.settings.lock_recover().clone()
    }

    /// Reloads settings and rebuilds the engine if the file changed since it was last read.
    /// One small read when it has not.
    fn sync_settings(&self) {
        let mut loaded_from = self.loaded_from.lock_recover();
        self.reload_if_changed(&mut loaded_from);
    }

    fn reload_if_changed(&self, loaded_from: &mut Option<Fingerprint>) {
        let now = Settings::fingerprint(&self.data_dir);
        if *loaded_from == now {
            return;
        }
        let loaded = Settings::load_checked(&self.data_dir);
        let engine = build_engine(&loaded.settings, loaded.problem.as_deref(), &self.data_dir);
        if let Some(problem) = &loaded.problem {
            tracing::warn!(%problem, "the settings file changed and cannot be read");
        }
        *self.engine.write_recover() = Arc::new(engine);
        *self.settings.lock_recover() = loaded.settings;
        *self.settings_problem.lock_recover() = loaded.problem;
        *loaded_from = now;
    }

    /// Changes the settings: reads what is on disk now, applies `change`, saves, and rebuilds the
    /// engine, all under one lock. A change is always made to the latest settings, so it cannot
    /// undo one the command line saved before it, and two changes from the app cannot interleave.
    ///
    /// If the file on disk could not be read, a copy of it is kept aside first: the user may
    /// still want what was in it, and the settings they save now become the ones that count.
    /// The original stays until the save replaces it, so a failed save cannot leave no file.
    pub fn update_settings<E: From<prune_core::PruneError>>(
        &self,
        change: impl FnOnce(&mut Settings) -> Result<(), E>,
    ) -> Result<(), E> {
        let mut loaded_from = self.loaded_from.lock_recover();
        self.reload_if_changed(&mut loaded_from);
        let mut settings = self.settings.lock_recover().clone();
        change(&mut settings)?;

        let mut problem = self.settings_problem.lock_recover();
        if problem.is_some() {
            if let Some(aside) = Settings::set_aside_unreadable(&self.data_dir)? {
                tracing::warn!(path = %aside.display(), "kept the unreadable settings file");
            }
        }
        settings.save(&self.data_dir)?;
        *problem = None;
        *loaded_from = Settings::fingerprint(&self.data_dir);
        *self.engine.write_recover() = Arc::new(build_engine(&settings, None, &self.data_dir));
        *self.settings.lock_recover() = settings;
        Ok(())
    }
}

/// The engine for `settings`. While the settings file cannot be read, what the user protected is
/// unknown, so it refuses every removal.
fn build_engine(settings: &Settings, problem: Option<&str>, data_dir: &Path) -> PruneEngine {
    let mut engine = PruneEngine::configured(settings, data_dir);
    if let Some(problem) = problem {
        engine.refuse_all_removal(problem.to_string());
    }
    engine
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder Prune would remove if nothing protected it: inside the temporary folder.
    fn removable_folder() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("prune-state-test")
            .tempdir_in(std::env::temp_dir())
            .unwrap()
    }

    /// What `prune protect add` does: read the file, add, save it whole.
    fn protect_from_elsewhere(dir: &Path, path: &Path) {
        let mut settings = Settings::load(dir);
        settings
            .protected_paths
            .push(std::fs::canonicalize(path).unwrap());
        settings.save(dir).unwrap();
    }

    #[test]
    fn a_path_protected_from_the_command_line_is_honoured_without_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path());
        let victim = removable_folder();
        assert!(state.engine().policy().validate(victim.path()).is_ok());

        protect_from_elsewhere(dir.path(), victim.path());
        let refused = state.engine().policy().validate(victim.path());
        assert!(refused.is_err(), "{refused:?}");
    }

    #[test]
    fn saving_from_the_app_keeps_what_the_command_line_saved_meanwhile() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path());
        let victim = removable_folder();
        protect_from_elsewhere(dir.path(), victim.path());

        // The app changes something else; the change is made to what is on disk now.
        state
            .update_settings(|s| -> prune_core::Result<()> {
                s.project_roots = vec![std::env::temp_dir()];
                Ok(())
            })
            .unwrap();

        let saved = Settings::load(dir.path());
        assert_eq!(saved.protected_paths.len(), 1);
        assert!(state.engine().policy().validate(victim.path()).is_err());
    }

    #[test]
    fn a_settings_file_broken_while_the_app_runs_stops_all_removal() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path());
        Settings::default().save(dir.path()).unwrap();
        let victim = removable_folder();
        assert!(state.engine().policy().validate(victim.path()).is_ok());

        std::fs::write(dir.path().join("settings.json"), b"{ \"protectedPaths\": [").unwrap();
        assert!(state.engine().policy().validate(victim.path()).is_err());
        assert!(state.settings_problem.lock_recover().is_some());
    }

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

        state
            .update_settings(|s| -> prune_core::Result<()> {
                *s = Settings::default();
                Ok(())
            })
            .unwrap();
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
