//! User settings, stored as one JSON file in the application data directory.
//!
//! Only choices the engine needs live here. Anything that is purely a matter of appearance
//! (theme, delete mode) stays in the frontend, because the engine has no use for it.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{PruneError, Result};

const FILE_NAME: &str = "settings.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Directories searched for project artifacts (`node_modules`, `target/`, …).
    ///
    /// Empty means "use the directories Prune guesses", which is what a fresh install does.
    /// Setting this is how a user points Prune at where their code actually lives, and how they
    /// keep it away from a huge tree they never want walked.
    pub project_roots: Vec<PathBuf>,
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

impl Settings {
    /// Reads the settings file. A missing or unreadable file yields the defaults rather than an
    /// error: settings are a convenience, and the app must still start without them.
    pub fn load(dir: &Path) -> Self {
        let path = dir.join(FILE_NAME);
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        match serde_json::from_str(&text) {
            Ok(settings) => settings,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "ignoring unreadable settings");
                Self::default()
            }
        }
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
    fn saving_replaces_the_file_in_one_step() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings {
            project_roots: vec![PathBuf::from("/home/u/code")],
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
        };
        first.save(dir.path()).unwrap();

        let second = Settings {
            project_roots: vec![PathBuf::from("/home/u/two")],
        };
        second.save(dir.path()).unwrap();

        assert_eq!(Settings::load(dir.path()), second);
    }

    #[test]
    fn round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings {
            project_roots: vec![PathBuf::from("/home/u/code")],
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
        };
        let roots = settings.sanitized_roots(&home);
        assert_eq!(roots.len(), 1);
        assert!(roots[0].ends_with("code"));
    }
}
