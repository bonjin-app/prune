use std::path::PathBuf;

use super::{
    existing, existing_project_roots, AppDataKind, ApplicationInfo, KnownPaths, PlatformService,
    ProtectedPaths, RelatedPath, StartupItem,
};
use crate::models::Platform;
use crate::Result;

mod apps;
mod startup;

pub struct MacosPlatform;

impl PlatformService for MacosPlatform {
    fn platform(&self) -> Platform {
        Platform::Macos
    }

    fn applications(&self, known: &KnownPaths) -> Result<Vec<ApplicationInfo>> {
        Ok(apps::list(known))
    }

    fn app_related_paths(&self, app: &ApplicationInfo, known: &KnownPaths) -> Vec<RelatedPath> {
        apps::related_paths(app, known)
    }

    fn startup_items(&self, known: &KnownPaths) -> Result<Vec<StartupItem>> {
        Ok(startup::list(known))
    }

    fn set_startup_enabled(&self, item: &StartupItem, enabled: bool) -> Result<()> {
        startup::set_enabled(item, enabled)
    }

    fn known_paths(&self) -> KnownPaths {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let library = home.join("Library");
        KnownPaths {
            user_cache: existing(library.join("Caches")),
            user_logs: existing(library.join("Logs")),
            app_support: existing(library.join("Application Support")),
            local_app_data: existing(library.join("Application Support")),
            temp: std::env::temp_dir(),
            trash: existing(home.join(".Trash")),
            downloads: existing(home.join("Downloads")),
            project_roots: existing_project_roots(&home),
            home,
        }
    }

    fn protected_paths(&self, known: &KnownPaths) -> ProtectedPaths {
        let home = &known.home;
        let lib = home.join("Library");
        ProtectedPaths {
            allowed_roots: vec![
                home.clone(),
                known.temp.clone(),
                PathBuf::from("/private/tmp"),
            ],
            exact: vec![
                home.clone(),
                lib.clone(),
                lib.join("Caches"),
                lib.join("Logs"),
                lib.join("Application Support"),
                lib.join("Preferences"),
                lib.join("Containers"),
                lib.join("Group Containers"),
                lib.join("Developer"),
                lib.join("Developer/Xcode"),
                lib.join("Developer/Xcode/DerivedData"),
                lib.join("Developer/CoreSimulator"),
                home.join(".Trash"),
                home.join("Documents"),
                home.join("Desktop"),
                home.join("Downloads"),
                home.join("Pictures"),
                home.join("Movies"),
                home.join("Music"),
                home.join("Applications"),
                home.join("Public"),
                known.temp.clone(),
            ],
            trees: vec![
                // system
                PathBuf::from("/System"),
                PathBuf::from("/Library"),
                PathBuf::from("/usr"),
                PathBuf::from("/bin"),
                PathBuf::from("/sbin"),
                PathBuf::from("/etc"),
                PathBuf::from("/private/etc"),
                PathBuf::from("/var/db"),
                PathBuf::from("/private/var/db"),
                PathBuf::from("/Applications"),
                PathBuf::from("/Volumes"),
                PathBuf::from("/opt"),
                PathBuf::from("/cores"),
                // user secrets & irreplaceable data
                home.join(".ssh"),
                home.join(".gnupg"),
                home.join(".aws"),
                home.join(".kube"),
                home.join(".config/gh"),
                lib.join("Keychains"),
                lib.join("Mail"),
                lib.join("Messages"),
                lib.join("Photos"),
                lib.join("Mobile Documents"),
                lib.join("Cookies"),
                lib.join("Safari"),
                lib.join("Accounts"),
                lib.join("Calendars"),
                lib.join("Reminders"),
                lib.join("Contacts"),
                lib.join("Passes"),
                lib.join("Wallet"),
                lib.join("Application Support/MobileSync"),
                lib.join("Application Support/AddressBook"),
                lib.join("Application Support/CloudDocs"),
                lib.join("Application Support/iCloud"),
                lib.join("Developer/Xcode/UserData"),
            ],
            app_bundle_roots: vec![PathBuf::from("/Applications"), home.join("Applications")],
        }
    }
}
