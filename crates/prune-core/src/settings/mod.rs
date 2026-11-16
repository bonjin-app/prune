//! User settings, stored as one JSON file in the application data directory.
//!
//! Only choices the engine or the desktop app's own process needs live here. Anything that is
//! purely a matter of appearance (the theme) stays in the frontend, because neither has a use
//! for it. The menu bar choices are here because the app needs them before any page loads. The delete mode is
//! kept nowhere: it is per run, so permanent deletion is never inherited from an earlier one.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{PruneError, Result};

const FILE_NAME: &str = "settings.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Directories searched for project artifacts (`node_modules`, `target/`, …).
    ///
    /// Empty means "use the directories Prune guesses", which is what a fresh install does.
    /// Setting this is how a user points Prune at where their code actually lives, and how they
    /// keep it away from a huge tree they never want walked.
    pub project_roots: Vec<PathBuf>,
    /// Paths Prune must never remove, nor remove anything that contains them.
    ///
    /// Kept exactly as chosen even when one stops existing (an unplugged drive, a renamed
    /// folder), so that it is protected again the moment it is back.
    pub protected_paths: Vec<PathBuf>,
    /// Show Prune's icon in the menu bar (the notification area on Windows) while it runs.
    pub menu_bar_icon: bool,
    /// Closing the window leaves Prune in the menu bar instead of quitting. Only while
    /// `menu_bar_icon` is on: a running app with no window and no icon could not be found.
    /// Prune never starts itself; this only keeps the copy the user opened.
    pub keep_in_menu_bar: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            project_roots: Vec::new(),
            protected_paths: Vec::new(),
            menu_bar_icon: true,
            keep_in_menu_bar: false,
        }
    }
}

impl Settings {
    /// Whether closing the window should leave Prune running in the menu bar.
    pub fn stays_in_menu_bar(&self) -> bool {
        self.menu_bar_icon && self.keep_in_menu_bar
    }
}

/// Settings as read from disk, and what went wrong reading them.
#[derive(Debug, Clone, Default)]
pub struct LoadedSettings {
    pub settings: Settings,
    /// Set when a settings file exists but could not be read. The protected paths in it are
    /// then unknown, so nothing may be removed until the file is fixed or replaced.
    pub problem: Option<String>,
}

/// Why a directory cannot be used as a project root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootRejection {
    NotAbsolute,
    Missing,
    NotADirectory,
    /// Prune can only remove things inside the home directory, so scanning outside it would
    /// only ever produce targets the safety layer refuses.
    OutsideHome,
}

impl RootRejection {
    pub fn message(self) -> &'static str {
        match self {
            RootRejection::NotAbsolute => "path must be absolute",
            RootRejection::Missing => "folder does not exist",
            RootRejection::NotADirectory => "path is not a folder",
            RootRejection::OutsideHome => "must be inside your home folder",
        }
    }
}

/// Checks one candidate project root, returning it in canonical form.
pub fn validate_root(path: &Path, home: &Path) -> std::result::Result<PathBuf, RootRejection> {
    if !path.is_absolute() {
        return Err(RootRejection::NotAbsolute);
    }
    let canonical = std::fs::canonicalize(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => RootRejection::Missing,
        _ => RootRejection::NotADirectory,
    })?;
    if !canonical.is_dir() {
        return Err(RootRejection::NotADirectory);
    }
    let home = std::fs::canonicalize(home).unwrap_or_else(|_| home.to_path_buf());
    if !canonical.starts_with(&home) {
        return Err(RootRejection::OutsideHome);
    }
    Ok(canonical)
}

/// Checks a path the user wants protected, returning it with the folders above it in canonical
/// form. It must exist when it is added, so a typo cannot pass for a protection.
///
/// The last component is kept as given rather than resolved: protecting a link protects the
/// link. The safety policy covers what it points to as well.
pub fn validate_protected(path: &Path) -> std::result::Result<PathBuf, RootRejection> {
    if !path.is_absolute() {
        return Err(RootRejection::NotAbsolute);
    }
    std::fs::symlink_metadata(path).map_err(|_| RootRejection::Missing)?;
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        // A filesystem root: nothing above it to resolve.
        return std::fs::canonicalize(path).map_err(|_| RootRejection::Missing);
    };
    let parent = std::fs::canonicalize(parent).map_err(|_| RootRejection::Missing)?;
    Ok(parent.join(name))
}

/// What identifies one version of the settings file: its length and a hash of its content.
/// `None` when there is no file.
///
/// Content rather than modification time: on volumes that keep times to the second or two
/// (HFS+, FAT, exFAT), removing one path and adding another of the same length within that
/// second would look like no change. The file is a few hundred bytes, so reading it is cheap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fingerprint {
    len: u64,
    hash: u64,
}

impl Settings {
    /// The settings file's current [`Fingerprint`], for noticing that the command line, or a
    /// second copy of the app, saved it.
    pub fn fingerprint(dir: &Path) -> Option<Fingerprint> {
        use std::hash::{Hash, Hasher};
        let bytes = std::fs::read(dir.join(FILE_NAME)).ok()?;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        bytes.hash(&mut hasher);
        Some(Fingerprint {
            len: bytes.len() as u64,
            hash: hasher.finish(),
        })
    }

    /// Reads the settings file. The app must start without one, so problems never fail this:
    /// a missing file yields the defaults, and an unreadable one yields the defaults with the
    /// problem described, for the caller to act on.
    pub fn load_checked(dir: &Path) -> LoadedSettings {
        let path = dir.join(FILE_NAME);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return LoadedSettings::default(),
            Err(e) => return LoadedSettings::unreadable(&path, e.to_string()),
        };
        match serde_json::from_str(&text) {
            Ok(settings) => LoadedSettings {
                settings,
                problem: None,
            },
            Err(e) => LoadedSettings::unreadable(&path, e.to_string()),
        }
    }

    /// [`load_checked`](Self::load_checked) without the problem.
    pub fn load(dir: &Path) -> Self {
        Self::load_checked(dir).settings
    }

    /// Copies an unreadable settings file aside, so that saving new settings does not destroy
    /// what the user had. Returns where the copy went.
    ///
    /// A copy, not a move: the original stays until [`save`](Self::save) replaces it. Moved
    /// away first, a save that then failed would leave no settings file at all, and the next
    /// start would read that as a fresh install with nothing protected.
    pub fn set_aside_unreadable(dir: &Path) -> Result<Option<PathBuf>> {
        let path = dir.join(FILE_NAME);
        if !path.exists() {
            return Ok(None);
        }
        let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        let aside = dir.join(format!("settings.unreadable-{stamp}.json"));
        std::fs::copy(&path, &aside).map_err(|e| PruneError::io(&path, e))?;
        Ok(Some(aside))
    }

    /// Writes the settings, replacing the file in one step.
    ///
    /// Writing in place truncates first, so a crash or a full disk between the truncate and the
    /// write leaves an empty file where the user's configuration was. Writing a temporary file
    /// and renaming it over the original means the file on disk is always either the old
    /// settings or the new ones, never half of either.
    pub fn save(&self, dir: &Path) -> Result<()> {
        std::fs::create_dir_all(dir).map_err(|e| PruneError::io(dir, e))?;
        let path = dir.join(FILE_NAME);
        let text = serde_json::to_string_pretty(self)?;

        let temp = path.with_extension("json.writing");
        std::fs::write(&temp, text).map_err(|e| PruneError::io(&temp, e))?;
        std::fs::rename(&temp, &path).map_err(|e| {
            // Do not leave the half-written file behind for the next run to trip over.
            let _ = std::fs::remove_file(&temp);
            PruneError::io(&path, e)
        })
    }

    /// Keeps only roots that are still usable, dropping duplicates and nested ones so no tree
    /// is walked twice.
    pub fn sanitized_roots(&self, home: &Path) -> Vec<PathBuf> {
        let mut roots: Vec<PathBuf> = Vec::new();
        for candidate in &self.project_roots {
            let Ok(path) = validate_root(candidate, home) else {
                continue;
            };
            if roots.iter().any(|kept| path.starts_with(kept)) {
                continue;
            }
            roots.retain(|kept| !kept.starts_with(&path));
            roots.push(path);
        }
        roots
    }
}

impl LoadedSettings {
    fn unreadable(path: &Path, error: String) -> Self {
        tracing::warn!(path = %path.display(), %error, "settings could not be read");
        Self {
            settings: Settings::default(),
            problem: Some(format!(
                "The settings file {} could not be read ({error}), so the folders you protected \
                 are unknown. Nothing will be removed until it is fixed, or until settings are \
                 saved again, which keeps the unreadable file beside the new one.",
                path.display()
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_yields_defaults() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Settings::load(dir.path()), Settings::default());
    }

    #[test]
    fn corrupt_file_yields_defaults_instead_of_failing_to_start() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE_NAME), b"{ not json").unwrap();
        assert_eq!(Settings::load(dir.path()), Settings::default());
    }

    #[test]
    fn a_corrupt_file_is_reported_not_silently_replaced() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE_NAME), b"{ not json").unwrap();
        let loaded = Settings::load_checked(dir.path());
        let problem = loaded.problem.expect("an unreadable file is a problem");
        assert!(problem.contains("could not be read"), "{problem}");
        // A missing file is not one: that is a fresh install.
        let fresh = tempfile::tempdir().unwrap();
        assert!(Settings::load_checked(fresh.path()).problem.is_none());
    }

    #[test]
    fn settings_from_before_the_menu_bar_show_the_icon_and_quit_on_close() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE_NAME), br#"{ "protectedPaths": [] }"#).unwrap();
        let settings = Settings::load(dir.path());
        assert!(settings.menu_bar_icon);
        assert!(!settings.stays_in_menu_bar());
    }

    #[test]
    fn staying_in_the_menu_bar_needs_the_icon() {
        let settings = Settings {
            menu_bar_icon: false,
            keep_in_menu_bar: true,
            ..Settings::default()
        };
        assert!(!settings.stays_in_menu_bar());
    }

    #[test]
    fn settings_from_before_protected_paths_still_load() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(FILE_NAME),
            br#"{ "projectRoots": ["/home/u/code"] }"#,
        )
        .unwrap();
        let loaded = Settings::load_checked(dir.path());
        assert!(loaded.problem.is_none());
        assert_eq!(
            loaded.settings.project_roots,
            vec![PathBuf::from("/home/u/code")]
        );
        assert!(loaded.settings.protected_paths.is_empty());
    }

    #[test]
    fn an_unreadable_file_is_set_aside_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE_NAME), b"{ broken").unwrap();
        let aside = Settings::set_aside_unreadable(dir.path())
            .unwrap()
            .expect("there was a file to set aside");
        assert_eq!(std::fs::read(&aside).unwrap(), b"{ broken");
        // Still there until a save replaces it, so a failed save cannot leave no file at all.
        assert!(Settings::load_checked(dir.path()).problem.is_some());
        Settings::default().save(dir.path()).unwrap();
        assert!(Settings::load_checked(dir.path()).problem.is_none());
        assert_eq!(std::fs::read(&aside).unwrap(), b"{ broken");
    }

    #[test]
    fn protected_paths_must_exist_when_added() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("keep")).unwrap();
        let canonical = std::fs::canonicalize(dir.path().join("keep")).unwrap();
        assert_eq!(validate_protected(&dir.path().join("keep")), Ok(canonical));
        assert_eq!(
            validate_protected(&dir.path().join("typo")),
            Err(RootRejection::Missing)
        );
        assert_eq!(
            validate_protected(Path::new("relative")),
            Err(RootRejection::NotAbsolute)
        );
    }

    #[test]
    fn saving_replaces_the_file_in_one_step() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings {
            project_roots: vec![PathBuf::from("/home/u/code")],
            ..Default::default()
        };
        settings.save(dir.path()).unwrap();

        // Nothing half-written is left lying about for the next run to read.
        let leftovers: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n != FILE_NAME)
            .collect();
        assert!(leftovers.is_empty(), "unexpected files: {leftovers:?}");
    }

    #[test]
    fn saving_over_existing_settings_keeps_them_readable_throughout() {
        let dir = tempfile::tempdir().unwrap();
        let first = Settings {
            project_roots: vec![PathBuf::from("/home/u/one")],
            ..Default::default()
        };
        first.save(dir.path()).unwrap();

        let second = Settings {
            project_roots: vec![PathBuf::from("/home/u/two")],
            ..Default::default()
        };
        second.save(dir.path()).unwrap();

        assert_eq!(Settings::load(dir.path()), second);
    }

    #[test]
    fn round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings {
            project_roots: vec![PathBuf::from("/home/u/code")],
            ..Default::default()
        };
        settings.save(dir.path()).unwrap();
        assert_eq!(Settings::load(dir.path()), settings);
    }

    #[test]
    fn validates_candidate_roots() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        std::fs::create_dir_all(home.join("code")).unwrap();
        std::fs::write(home.join("file.txt"), b"x").unwrap();
        std::fs::create_dir_all(dir.path().join("elsewhere")).unwrap();

        assert!(validate_root(&home.join("code"), &home).is_ok());
        assert_eq!(
            validate_root(Path::new("relative"), &home),
            Err(RootRejection::NotAbsolute)
        );
        assert_eq!(
            validate_root(&home.join("nope"), &home),
            Err(RootRejection::Missing)
        );
        assert_eq!(
            validate_root(&home.join("file.txt"), &home),
            Err(RootRejection::NotADirectory)
        );
        assert_eq!(
            validate_root(&dir.path().join("elsewhere"), &home),
            Err(RootRejection::OutsideHome)
        );
    }

    #[test]
    fn sanitizing_drops_unusable_and_nested_roots() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        std::fs::create_dir_all(home.join("code/app")).unwrap();
        std::fs::create_dir_all(home.join("work")).unwrap();

        let settings = Settings {
            project_roots: vec![
                home.join("code"),
                // Already covered by `code`, so it must not be walked twice.
                home.join("code/app"),
                home.join("gone"),
                home.join("work"),
            ],
            ..Default::default()
        };
        let names: Vec<String> = settings
            .sanitized_roots(&home)
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["code", "work"]);
    }

    #[test]
    fn a_parent_added_later_replaces_the_child() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        std::fs::create_dir_all(home.join("code/app")).unwrap();
        let settings = Settings {
            project_roots: vec![home.join("code/app"), home.join("code")],
            ..Default::default()
        };
        let roots = settings.sanitized_roots(&home);
        assert_eq!(roots.len(), 1);
        assert!(roots[0].ends_with("code"));
    }
}
