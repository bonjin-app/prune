use std::path::PathBuf;

use super::{
    existing, existing_project_roots, AppDataKind, ApplicationInfo, KnownPaths, PlatformService,
    ProtectedPaths, RelatedPath, StartupItem,
};
use crate::models::Platform;
use crate::Result;

mod apps;
mod startup;

pub struct WindowsPlatform;

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
}

impl PlatformService for WindowsPlatform {
    fn platform(&self) -> Platform {
        Platform::Windows
    }

    fn applications(&self, known: &KnownPaths) -> Result<Vec<ApplicationInfo>> {
        apps::list(known)
    }

    fn app_related_paths(&self, app: &ApplicationInfo, known: &KnownPaths) -> Vec<RelatedPath> {
        apps::related_paths(app, known)
    }

    fn startup_items(&self, known: &KnownPaths) -> Result<Vec<StartupItem>> {
        startup::list(known)
    }

    fn set_startup_enabled(&self, item: &StartupItem, enabled: bool) -> Result<()> {
        startup::set_enabled(item, enabled)
    }

    fn known_paths(&self) -> KnownPaths {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("C:\\"));
        let local = env_path("LOCALAPPDATA").or_else(dirs::cache_dir);
        let roaming = env_path("APPDATA").or_else(dirs::config_dir);
        KnownPaths {
            user_cache: local.clone().and_then(existing),
            user_logs: None,
            app_support: roaming.and_then(existing),
            local_app_data: local.and_then(existing),
            temp: std::env::temp_dir(),
            // The Recycle Bin is per-volume and hidden; handled by the platform, not as a path.
            trash: None,
            downloads: dirs::download_dir(),
            project_roots: existing_project_roots(&home),
            home,
        }
    }

    fn protected_paths(&self, known: &KnownPaths) -> ProtectedPaths {
        let home = &known.home;
        let system_drive = env_path("SystemDrive").unwrap_or_else(|| PathBuf::from("C:"));
        let windir = env_path("WINDIR").unwrap_or_else(|| system_drive.join("Windows"));
        let program_files =
            env_path("ProgramFiles").unwrap_or_else(|| system_drive.join("Program Files"));
        let program_files_x86 = env_path("ProgramFiles(x86)")
            .unwrap_or_else(|| system_drive.join("Program Files (x86)"));
        let program_data =
            env_path("ProgramData").unwrap_or_else(|| system_drive.join("ProgramData"));
        let local = known
            .local_app_data
            .clone()
            .unwrap_or_else(|| home.join("AppData\\Local"));
        let roaming = known
            .app_support
            .clone()
            .unwrap_or_else(|| home.join("AppData\\Roaming"));

        ProtectedPaths {
            allowed_roots: vec![home.clone(), known.temp.clone()],
            exact: vec![
                home.clone(),
                home.join("AppData"),
                local.clone(),
                roaming.clone(),
                home.join("AppData\\LocalLow"),
                local.join("Temp"),
                home.join("Documents"),
                home.join("Desktop"),
                home.join("Downloads"),
                home.join("Pictures"),
                home.join("Videos"),
                home.join("Music"),
                home.join("OneDrive"),
                known.temp.clone(),
            ],
            trees: vec![
                windir,
                program_files,
                program_files_x86,
                program_data,
                system_drive.join("System Volume Information"),
                system_drive.join("$Recycle.Bin"),
                system_drive.join("Recovery"),
                system_drive.join("Boot"),
                home.join(".ssh"),
                home.join(".gnupg"),
                home.join(".aws"),
                home.join(".kube"),
                home.join("NTUSER.DAT"),
                local.join("Microsoft\\Credentials"),
                roaming.join("Microsoft\\Credentials"),
                roaming.join("Microsoft\\Crypto"),
                roaming.join("Microsoft\\Protect"),
                roaming.join("Microsoft\\SystemCertificates"),
                local.join("Microsoft\\Windows"),
                local.join("Packages"),
                home.join("OneDrive"),
            ],
            app_bundle_roots: vec![],
        }
    }
}
