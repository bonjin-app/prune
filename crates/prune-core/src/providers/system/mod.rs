//! Generic system-level providers: caches, logs, temp files, trash, old installers.

use std::path::PathBuf;

use super::{Root, SimpleProvider};
use crate::models::{Category, RiskLevel};
use crate::platform::KnownPaths;

/// Per-user application caches. Low priority so specialised providers win on overlap.
pub const USER_CACHE: SimpleProvider = SimpleProvider::new(
    "user_cache",
    "Application Caches",
    Category::ApplicationCache,
    "Per-application cache folders. Apps rebuild them on next launch.",
    RiskLevel::Safe,
    user_cache_roots,
)
.priority(10)
.exclude(&[
    // iCloud / account state that is expensive or impossible to rebuild.
    "CloudKit",
    "com.apple.bird",
    "com.apple.HomeKit",
    "com.apple.akd",
    "com.apple.passd",
    "com.apple.iCloudHelper",
    "com.apple.Safari",
    "com.apple.Safari.SafeBrowsing",
    "FamilyCircle",
    "com.apple.ap.adprivacyd",
    "com.apple.containermanagerd",
    "com.apple.nsurlsessiond",
]);

fn user_cache_roots(known: &KnownPaths) -> Vec<Root> {
    if cfg!(target_os = "windows") {
        // Windows has no unified cache directory; `%LOCALAPPDATA%` holds application *data*.
        // Only well-known, regenerable cache locations are listed.
        let Some(local) = known.local_app_data.as_ref() else {
            return vec![];
        };
        vec![
            Root::labelled(
                local.join("Microsoft\\Windows\\INetCache"),
                "Internet Cache",
            ),
            Root::labelled(
                local.join("Microsoft\\Windows\\WER"),
                "Windows Error Reports",
            ),
            Root::labelled(local.join("D3DSCache"), "DirectX Shader Cache"),
            Root::labelled(local.join("NVIDIA\\DXCache"), "NVIDIA Shader Cache"),
            Root::labelled(local.join("NVIDIA\\GLCache"), "NVIDIA GL Cache"),
            Root::labelled(local.join("AMD\\DxCache"), "AMD Shader Cache"),
            Root::labelled(local.join("CrashDumps"), "Crash Dumps"),
        ]
    } else {
        known.user_cache.iter().map(Root::children).collect()
    }
}

pub const USER_LOGS: SimpleProvider = SimpleProvider::new(
    "user_logs",
    "Logs",
    Category::Logs,
    "Diagnostic logs written by applications. Safe to remove; useful only when debugging.",
    RiskLevel::Safe,
    |known| known.user_logs.iter().map(Root::children).collect(),
)
.priority(10);

pub const TRASH: SimpleProvider = SimpleProvider::new(
    "trash",
    "Trash",
    Category::Trash,
    "Items already in the Trash. Removing them frees the space for good.",
    RiskLevel::Safe,
    |known| known.trash.iter().map(Root::children).collect(),
)
.permanent_only();

pub const TEMP_FILES: SimpleProvider = SimpleProvider::new(
    "temp_files",
    "Temporary Files",
    Category::TemporaryFiles,
    "Temporary files older than a day. Running programs may still use newer ones.",
    RiskLevel::Low,
    |known| vec![Root::children(known.temp.clone())],
)
.priority(20)
.min_age_days(1)
.exclude(&["com.apple.launchd", "powerlog", ".X11-unix", ".ICE-unix"]);

/// Installer images / packages sitting in Downloads.
pub struct OldInstallers;

impl OldInstallers {
    /// Only formats that exist to install something. A `.zip` was deliberately dropped: it is
    /// as likely to be a download the user meant to keep as an installer, and Prune should not
    /// be the reason someone loses one.
    const EXTENSIONS: &'static [&'static str] = if cfg!(target_os = "windows") {
        &["msi", "msix", "appx", "appxbundle", "iso"]
    } else {
        &["dmg", "pkg", "xip", "iso"]
    };
}

impl super::CleanupProvider for OldInstallers {
    fn id(&self) -> &str {
        "old_installers"
    }
    fn name(&self) -> &str {
        "Old Installers"
    }
    fn category(&self) -> Category {
        Category::OldInstallers
    }
    fn description(&self) -> &str {
        "Installer images and packages in Downloads that are older than a week."
    }
    fn default_risk(&self) -> RiskLevel {
        RiskLevel::Low
    }
    fn is_available(&self, known: &KnownPaths) -> bool {
        known.downloads.as_ref().is_some_and(|d| d.is_dir())
    }
    fn scan(&self, ctx: &super::ScanContext<'_>) -> super::ScanOutput {
        let mut out = super::ScanOutput::default();
        let Some(downloads) = ctx.known.downloads.as_ref() else {
            return out;
        };
        let Ok(entries) = std::fs::read_dir(downloads) else {
            return out;
        };
        let week = std::time::Duration::from_secs(7 * 86_400);
        let mut files: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .filter(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| Self::EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
            })
            .filter(|p| {
                std::fs::metadata(p)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|m| std::time::SystemTime::now().duration_since(m).ok())
                    .is_some_and(|age| age >= week)
            })
            .collect();
        files.sort();
        for file in files {
            if ctx.is_cancelled() {
                break;
            }
            let label = super::simple::file_name(&file);
            out.push_measured(ctx, self.id(), &file, label, RiskLevel::Low, None, false);
        }
        out
    }
}
