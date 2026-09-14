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
pub mod startup_approved;

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
    /// Directories whose *direct* `.app` children may be removed (macOS `/Applications`,
    /// `~/Applications`) even though they are outside `allowed_roots` or inside a protected
    /// tree. Only the bundle itself qualifies, never the directory or anything inside a bundle.
    pub app_bundle_roots: Vec<PathBuf>,
}

/// Installed application (Phase 6 – Uninstaller).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationInfo {
    /// Stable id derived from the path (macOS) or registry key (Windows).
    pub id: String,
    pub name: String,
    /// Bundle path (macOS) or install location (Windows, may be empty).
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// `CFBundleIdentifier` on macOS; the registry key name on Windows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    /// `None` until measured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Where the entry came from: `applications`, `user_applications`, `registry`.
    pub source: String,
    /// Apple / Microsoft system component. Listed but never removable.
    pub is_system: bool,
    /// Windows: the vendor uninstaller command line. `None` on macOS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uninstall_command: Option<String>,
}

/// Kind of data an application leaves behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppDataKind {
    Application,
    Caches,
    ApplicationSupport,
    Preferences,
    Logs,
    Containers,
    SavedState,
    WebKit,
    HttpStorages,
    LaunchAgents,
    ApplicationScripts,
    CrashReports,
    LocalAppData,
    RoamingAppData,
}

impl AppDataKind {
    pub fn label(self) -> &'static str {
        match self {
            AppDataKind::Application => "Application",
            AppDataKind::Caches => "Caches",
            AppDataKind::ApplicationSupport => "Application Support",
            AppDataKind::Preferences => "Preferences",
            AppDataKind::Logs => "Logs",
            AppDataKind::Containers => "Containers",
            AppDataKind::SavedState => "Saved State",
            AppDataKind::WebKit => "WebKit Storage",
            AppDataKind::HttpStorages => "HTTP Storage",
            AppDataKind::LaunchAgents => "Launch Agents",
            AppDataKind::ApplicationScripts => "Application Scripts",
            AppDataKind::CrashReports => "Crash Reports",
            AppDataKind::LocalAppData => "Local App Data",
            AppDataKind::RoamingAppData => "Roaming App Data",
        }
    }

    /// Default risk of removing this kind of data.
    pub fn risk(self) -> crate::models::RiskLevel {
        use crate::models::RiskLevel::*;
        match self {
            AppDataKind::Application => Medium,
            AppDataKind::Caches
            | AppDataKind::Logs
            | AppDataKind::SavedState
            | AppDataKind::WebKit
            | AppDataKind::HttpStorages
            | AppDataKind::CrashReports => Safe,
            AppDataKind::Preferences => Low,
            AppDataKind::ApplicationSupport
            | AppDataKind::Containers
            | AppDataKind::LaunchAgents
            | AppDataKind::ApplicationScripts
            | AppDataKind::LocalAppData
            | AppDataKind::RoamingAppData => Medium,
        }
    }
}

/// A candidate location of application data. Only existing paths are returned.
#[derive(Debug, Clone)]
pub struct RelatedPath {
    pub kind: AppDataKind,
    pub path: PathBuf,
}

/// Who a startup item belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupScope {
    /// Runs for this user only, and can be changed without administrator rights.
    User,
    /// Installed for every user. Changing it needs administrator rights, so Prune shows it
    /// read-only rather than asking for a password.
    System,
}

/// When an item actually runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupTrigger {
    /// Starts at login / boot.
    AtLogin,
    /// Restarted whenever it exits.
    KeepAlive,
    /// Runs on a timer.
    Scheduled,
    /// Only starts when something asks for it (sockets, file watches, XPC).
    OnDemand,
}

/// Whether a permission has been granted to the running process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionState {
    Granted,
    Denied,
    /// The OS has no such permission, so nothing is missing.
    NotApplicable,
}

/// What the running process is and is not allowed to read.
///
/// Without this, a scan silently under-reports: macOS refuses `~/.Trash`, Safari's data and a
/// few other locations to processes without Full Disk Access, and the user sees a smaller
/// number with no explanation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Permissions {
    pub full_disk_access: PermissionState,
    /// Human-readable names of locations that were refused, for the UI to list.
    pub blocked: Vec<String>,
    /// Short instruction for granting it, or `None` when nothing is missing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub how_to_grant: Option<String>,
}

impl Permissions {
    pub fn not_applicable() -> Self {
        Self {
            full_disk_access: PermissionState::NotApplicable,
            blocked: Vec::new(),
            how_to_grant: None,
        }
    }
}

/// A program that starts by itself (Phase 7 – Startup Manager).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupItem {
    /// Stable id derived from the source and the item's location.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// launchd label (macOS) or registry value name (Windows).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The plist, shortcut or registry key that defines the item.
    pub path: String,
    /// The program that gets executed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    pub enabled: bool,
    /// `launch_agent`, `launch_daemon`, `registry_run`, `startup_folder`.
    pub source: String,
    pub scope: StartupScope,
    pub trigger: StartupTrigger,
    /// Whether Prune can change this item without administrator rights.
    pub can_toggle: bool,
    /// Why it cannot be toggled, when `can_toggle` is false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// The contract every supported OS implements.
pub trait PlatformService: Send + Sync {
    fn platform(&self) -> Platform;
    fn known_paths(&self) -> KnownPaths;
    fn protected_paths(&self, known: &KnownPaths) -> ProtectedPaths;
    /// Installed applications without sizes (fast). Default: not implemented.
    fn applications(&self, known: &KnownPaths) -> Result<Vec<ApplicationInfo>> {
        let _ = known;
        Err(crate::PruneError::NotImplemented("applications"))
    }
    /// Existing locations where `app` keeps data outside its bundle / install dir.
    fn app_related_paths(&self, app: &ApplicationInfo, known: &KnownPaths) -> Vec<RelatedPath> {
        let _ = (app, known);
        Vec::new()
    }
    /// What the process may read. Cheap: a few `read_dir` probes.
    fn permissions(&self, known: &KnownPaths) -> Permissions {
        let _ = known;
        Permissions::not_applicable()
    }
    /// Opens the OS settings pane where the missing permission is granted.
    fn open_privacy_settings(&self) -> Result<()> {
        Err(crate::PruneError::NotImplemented("open_privacy_settings"))
    }
    /// Programs that start by themselves. Default: not implemented.
    fn startup_items(&self, known: &KnownPaths) -> Result<Vec<StartupItem>> {
        let _ = known;
        Err(crate::PruneError::NotImplemented("startup_items"))
    }
    /// Enable or disable a startup item. Never deletes anything; the change is reversible.
    fn set_startup_enabled(&self, item: &StartupItem, enabled: bool) -> Result<()> {
        let _ = (item, enabled);
        Err(crate::PruneError::NotImplemented("set_startup_enabled"))
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
