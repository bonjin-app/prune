//! OS specific knowledge, hidden behind [`PlatformService`].
//!
//! Everything above this module (providers, safety, UI) must be OS agnostic and ask the
//! platform for well-known directories instead of hard-coding them.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::models::Platform;
use crate::Result;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub mod generic;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

/// Well-known user directories for the current OS. Missing directories are simply `None`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownPaths {
    pub home: PathBuf,
    /// Per-user cache root (`~/Library/Caches`, `%LOCALAPPDATA%`).
    pub user_cache: Option<PathBuf>,
    /// Per-user log root (`~/Library/Logs`).
    pub user_logs: Option<PathBuf>,
    /// Per-user application data (`~/Library/Application Support`, `%APPDATA%`).
    pub app_support: Option<PathBuf>,
    /// Local (non-roaming) application data. Same as `app_support` on macOS.
    pub local_app_data: Option<PathBuf>,
    pub temp: PathBuf,
    pub trash: Option<PathBuf>,
    pub downloads: Option<PathBuf>,
    /// Directories that commonly hold source code checkouts. Only existing ones are listed.
    pub project_roots: Vec<PathBuf>,
}

/// Paths the safety layer must refuse to touch.
#[derive(Debug, Clone, Default)]
pub struct ProtectedPaths {
    /// Roots Prune is *allowed* to delete inside. Everything else is refused.
    pub allowed_roots: Vec<PathBuf>,
    /// Exact paths that must never be removed themselves (but children may be).
    pub exact: Vec<PathBuf>,
    /// Whole trees that must never be touched, including all descendants.
    pub trees: Vec<PathBuf>,
}

/// Installed application (Phase 6 – Uninstaller).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationInfo {
    pub name: String,
    pub path: String,
    pub version: Option<String>,
    pub bundle_id: Option<String>,
    pub size_bytes: u64,
}

/// Login / startup item (Phase 7 – Startup Manager).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupItem {
    pub id: String,
    pub name: String,
    pub path: String,
    pub enabled: bool,
    pub source: String,
}

/// The contract every supported OS implements.
pub trait PlatformService: Send + Sync {
    fn platform(&self) -> Platform;
    fn known_paths(&self) -> KnownPaths;
    fn protected_paths(&self, known: &KnownPaths) -> ProtectedPaths;
    /// Phase 6. Default implementation reports "not implemented".
    fn applications(&self) -> Result<Vec<ApplicationInfo>> {
        Err(crate::PruneError::NotImplemented("applications"))
    }
    /// Phase 7. Default implementation reports "not implemented".
    fn startup_items(&self) -> Result<Vec<StartupItem>> {
        Err(crate::PruneError::NotImplemented("startup_items"))
    }
}

/// The platform service for the OS this binary was compiled for.
pub fn current() -> Box<dyn PlatformService> {
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacosPlatform)
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::WindowsPlatform)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Box::new(generic::GenericPlatform)
    }
}

/// Candidate project directories shared by all platforms; only existing ones are returned.
pub(crate) fn existing_project_roots(home: &std::path::Path) -> Vec<PathBuf> {
    const CANDIDATES: &[&str] = &[
        "Projects",
        "projects",
        "Developer",
        "dev",
        "Dev",
        "workspace",
        "Workspace",
        "src",
        "code",
        "Code",
        "repos",
        "git",
        "GitHub",
        "Documents/GitHub",
        "Documents/Projects",
        "Documents/projects",
        "Documents/workspace",
        "Documents/dev",
        "Documents/code",
        "Desktop",
    ];
    CANDIDATES
        .iter()
        .map(|c| home.join(c))
        .filter(|p| p.is_dir())
        .collect()
}

/// Helper shared by platform implementations.
pub(crate) fn existing(path: PathBuf) -> Option<PathBuf> {
    if path.exists() {
        Some(path)
    } else {
        None
    }
}
