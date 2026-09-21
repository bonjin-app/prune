//! Which folders under `AppData` belong to an application.
//!
//! Separate from [`super::apps`] because none of it needs Windows: it is path logic, it decides
//! what an uninstall offers to delete, and getting it wrong loses somebody's data. Compiling it
//! everywhere means it is tested on every machine and every CI job, not only on the one
//! platform it ships to — which matters more here than usual, because Windows has never been
//! run by hand during development.

use std::path::PathBuf;

use crate::platform::{AppDataKind, ApplicationInfo, KnownPaths, RelatedPath};

/// Folder names under `AppData` that belong to a vendor, to Windows, or to every application
/// at once.
///
/// Matching by folder name is what finds `%APPDATA%\\Slack` for Slack, but a name is not proof
/// of ownership. `%LOCALAPPDATA%\\Programs` is where per-user installations live — every one of
/// them — and it cannot be added to the protected list, because the applications inside it are
/// exactly what the uninstaller is for. `%APPDATA%\\Microsoft` holds Office templates, mail
/// signatures, the Start Menu and the recent-files list. Handing either to one application
/// because its name happened to match is the mistake this list exists to prevent; the same list
/// on macOS is what stops one Google application claiming Chrome's profiles.
const SHARED_FOLDERS: &[&str] = &[
    // Vendor umbrellas.
    "microsoft",
    "google",
    "mozilla",
    "adobe",
    "apple",
    "jetbrains",
    "oracle",
    "nvidia",
    "intel",
    "amd",
    "realtek",
    // Windows' own, and things every application writes into.
    "temp",
    "tmp",
    "programs",
    "packages",
    "publishers",
    "virtualstore",
    "cache",
    "caches",
    "logs",
    "crashdumps",
    "crashpad",
    "connecteddevicesplatform",
    "elevateddiagnostics",
    "d3dscache",
    "iconcache",
    "history",
    "recent",
    "startmenu",
];

/// `true` when a name is long enough and plain enough to look for as a folder at all.
fn specific_enough(name: &str) -> bool {
    let name = name.trim();
    name.len() >= 3 && !name.contains(['/', '\\'])
}

/// `true` when a folder of this name, directly under `AppData`, may be treated as this
/// application's own.
///
/// Short names match too much, and shared folders belong to no single application.
fn claimable(name: &str) -> bool {
    specific_enough(name) && !SHARED_FOLDERS.contains(&name.trim().to_ascii_lowercase().as_str())
}

pub fn related_paths(app: &ApplicationInfo, known: &KnownPaths) -> Vec<RelatedPath> {
    let mut out: Vec<RelatedPath> = Vec::new();
    let mut push = |kind: AppDataKind, path: PathBuf| {
        if path.exists() && !out.iter().any(|r| r.path == path) {
            out.push(RelatedPath { kind, path });
        }
    };
    if !app.path.is_empty() {
        let p = PathBuf::from(&app.path);
        // Install locations under the user profile can be removed; Program Files cannot
        // (the vendor uninstaller handles those) and would be marked Protected anyway.
        push(AppDataKind::Application, p);
    }
    let name = app.name.trim();
    if claimable(name) {
        if let Some(local) = &known.local_app_data {
            push(AppDataKind::LocalAppData, local.join(name));
        }
        if let Some(roaming) = &known.app_support {
            push(AppDataKind::RoamingAppData, roaming.join(name));
        }
    }
    // `Vendor\Product` is this application's own folder even when the vendor folder above it is
    // shared, so it is looked for separately — `%APPDATA%\Microsoft` belongs to nobody, but
    // `%APPDATA%\Microsoft\Teams` belongs to Teams.
    let publisher = app.publisher.as_deref().map(str::trim).unwrap_or_default();
    if !publisher.is_empty() && specific_enough(name) && !publisher.contains(['/', '\\']) {
        if let Some(local) = &known.local_app_data {
            push(AppDataKind::LocalAppData, local.join(publisher).join(name));
        }
        if let Some(roaming) = &known.app_support {
            push(
                AppDataKind::RoamingAppData,
                roaming.join(publisher).join(name),
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Windows is built and tested in CI but has never run on real hardware here, so the rules
    /// that decide what belongs to an application are pinned by tests rather than by trying it.
    fn app(name: &str, publisher: Option<&str>) -> ApplicationInfo {
        ApplicationInfo {
            id: "id".into(),
            name: name.into(),
            path: String::new(),
            version: None,
            bundle_id: Some("key".into()),
            publisher: publisher.map(str::to_string),
            size_bytes: None,
            modified_at: None,
            source: "registry".into(),
            is_system: false,
            uninstall_command: None,
        }
    }

    /// `KnownPaths` whose AppData folders live in a temp directory.
    fn sandbox(dir: &std::path::Path) -> KnownPaths {
        let local = dir.join("AppData/Local");
        let roaming = dir.join("AppData/Roaming");
        std::fs::create_dir_all(&local).unwrap();
        std::fs::create_dir_all(&roaming).unwrap();
        KnownPaths {
            user_cache: Some(local.clone()),
            user_logs: None,
            app_support: Some(roaming),
            local_app_data: Some(local),
            temp: dir.join("Temp"),
            trash: None,
            downloads: None,
            project_roots: vec![],
            home: dir.to_path_buf(),
        }
    }

    fn names_found(app: &ApplicationInfo, known: &KnownPaths) -> Vec<String> {
        related_paths(app, known)
            .into_iter()
            .map(|r| r.path.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    #[test]
    fn an_applications_own_folder_is_found_in_both_appdata_roots() {
        let dir = tempfile::tempdir().unwrap();
        let known = sandbox(dir.path());
        std::fs::create_dir_all(known.local_app_data.as_ref().unwrap().join("Slack")).unwrap();
        std::fs::create_dir_all(known.app_support.as_ref().unwrap().join("Slack")).unwrap();

        let found = names_found(&app("Slack", Some("Slack Technologies")), &known);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found.iter().all(|p| p.ends_with("/Slack")));
    }

    #[test]
    fn a_vendor_folder_is_never_claimed_by_one_application() {
        let dir = tempfile::tempdir().unwrap();
        let known = sandbox(dir.path());
        let roaming = known.app_support.as_ref().unwrap();
        // What `%APPDATA%\Microsoft` really holds: other products' data and Windows' own.
        std::fs::create_dir_all(roaming.join("Microsoft/Templates")).unwrap();
        std::fs::create_dir_all(roaming.join("Microsoft/Signatures")).unwrap();
        std::fs::create_dir_all(roaming.join("Microsoft/Windows/Recent")).unwrap();

        let found = names_found(&app("Microsoft", Some("Microsoft Corporation")), &known);
        assert!(
            found.is_empty(),
            "a vendor folder is not one application's to remove: {found:?}"
        );
    }

    #[test]
    fn the_folder_every_per_user_installation_lives_in_is_never_claimed() {
        let dir = tempfile::tempdir().unwrap();
        let known = sandbox(dir.path());
        let local = known.local_app_data.as_ref().unwrap();
        // This one cannot be added to the protected list: the applications inside it are
        // exactly what the uninstaller exists to remove.
        std::fs::create_dir_all(local.join("Programs/Microsoft VS Code")).unwrap();
        std::fs::create_dir_all(local.join("Programs/Discord")).unwrap();

        for name in ["Programs", "Packages", "Temp", "Cache", "Logs"] {
            std::fs::create_dir_all(local.join(name)).unwrap();
            let found = names_found(&app(name, Some("Some Vendor")), &known);
            assert!(
                found.is_empty(),
                "{name} should not be claimable: {found:?}"
            );
        }
    }

    #[test]
    fn a_product_folder_inside_a_vendor_folder_is_still_found() {
        let dir = tempfile::tempdir().unwrap();
        let known = sandbox(dir.path());
        let roaming = known.app_support.as_ref().unwrap();
        std::fs::create_dir_all(roaming.join("Microsoft/Teams")).unwrap();
        std::fs::create_dir_all(roaming.join("Microsoft/Signatures")).unwrap();

        let found = names_found(&app("Teams", Some("Microsoft")), &known);
        assert_eq!(
            found,
            vec![format!(
                "{}/Microsoft/Teams",
                roaming.to_string_lossy().replace('\\', "/")
            )]
        );
    }

    #[test]
    fn short_names_match_too_much_to_be_used() {
        let dir = tempfile::tempdir().unwrap();
        let known = sandbox(dir.path());
        std::fs::create_dir_all(known.app_support.as_ref().unwrap().join("Go")).unwrap();

        assert!(names_found(&app("Go", Some("Google")), &known).is_empty());
        assert!(!claimable("Go"));
        assert!(claimable("Godot"));
    }

    #[test]
    fn a_name_with_a_separator_in_it_cannot_reach_outside_appdata() {
        // The registry is vendor-supplied text, so a DisplayName is not a safe path component.
        for hostile in [r"..\..\Windows", "Foo/Bar", r"Foo\Bar"] {
            assert!(!claimable(hostile), "{hostile} must not be claimable");
            assert!(!specific_enough(hostile), "{hostile} is not a plain name");
        }
    }

    #[test]
    fn an_install_location_is_offered_but_still_has_to_pass_the_safety_layer() {
        let dir = tempfile::tempdir().unwrap();
        let known = sandbox(dir.path());
        let install = dir.path().join("AppData/Local/Programs/Thing");
        std::fs::create_dir_all(&install).unwrap();

        let mut a = app("Thing", Some("Vendor"));
        a.path = install.to_string_lossy().into_owned();
        let found = related_paths(&a, &known);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, AppDataKind::Application);
        assert_eq!(found[0].path, install);
    }
}
