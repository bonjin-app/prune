//! Fallback used on Linux (not an official target yet) so the crate still compiles and tests.

use std::path::PathBuf;

use super::{existing, existing_project_roots, KnownPaths, PlatformService, ProtectedPaths};
use crate::models::Platform;

pub struct GenericPlatform;

impl PlatformService for GenericPlatform {
    fn platform(&self) -> Platform {
        Platform::current()
    }

    fn known_paths(&self) -> KnownPaths {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        KnownPaths {
            user_cache: dirs::cache_dir().and_then(existing),
            user_logs: None,
            app_support: dirs::data_dir().and_then(existing),
            local_app_data: dirs::data_local_dir().and_then(existing),
            temp: std::env::temp_dir(),
            trash: dirs::data_dir()
                .map(|d| d.join("Trash/files"))
                .and_then(existing),
            downloads: dirs::download_dir(),
            project_roots: existing_project_roots(&home),
            home,
        }
    }

    fn protected_paths(&self, known: &KnownPaths) -> ProtectedPaths {
        let home = &known.home;
        ProtectedPaths {
            allowed_roots: vec![home.clone(), known.temp.clone()],
            exact: vec![
                home.clone(),
                home.join(".cache"),
                home.join(".config"),
                home.join(".local"),
                home.join(".local/share"),
                known.temp.clone(),
            ],
            trees: vec![
                PathBuf::from("/usr"),
                PathBuf::from("/bin"),
                PathBuf::from("/sbin"),
                PathBuf::from("/etc"),
                PathBuf::from("/var"),
                PathBuf::from("/boot"),
                PathBuf::from("/lib"),
                PathBuf::from("/opt"),
                home.join(".ssh"),
                home.join(".gnupg"),
            ],
        }
    }
}
