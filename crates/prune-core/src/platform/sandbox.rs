//! A platform whose every path lives under one directory.
//!
//! Anything that touches the filesystem has to be testable without pointing at a real home
//! directory. This implementation maps [`KnownPaths`] into a temporary tree so a test can lay
//! out caches, projects and protected data by hand and then run the whole scan → plan →
//! execute pipeline against it.
//!
//! It is part of the public API on purpose: the CLI's tests use it too, and so can anyone
//! writing a cleanup provider.

use std::path::{Path, PathBuf};

use super::{KnownPaths, PlatformService, ProtectedPaths};
use crate::models::Platform;

/// Platform rooted at a directory of the caller's choosing.
#[derive(Debug, Clone)]
pub struct SandboxPlatform {
    root: PathBuf,
}

impl SandboxPlatform {
    /// `root` is the parent of the fake `home` and `tmp` directories.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn home(&self) -> PathBuf {
        self.root.join("home")
    }

    pub fn temp(&self) -> PathBuf {
        self.root.join("tmp")
    }

    /// Creates the directories the sandbox describes, so a test can start writing files.
    pub fn prepare(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.home().join("Library/Caches"))?;
        std::fs::create_dir_all(self.home().join("Library/Logs"))?;
        std::fs::create_dir_all(self.home().join("Library/Application Support"))?;
        std::fs::create_dir_all(self.home().join(".Trash"))?;
        std::fs::create_dir_all(self.home().join("Downloads"))?;
        std::fs::create_dir_all(self.home().join("Projects"))?;
        std::fs::create_dir_all(self.temp())?;
        Ok(())
    }

    /// Writes a file of `size` bytes, creating parents. Convenience for tests.
    pub fn write(
        &self,
        relative_to_home: impl AsRef<Path>,
        size: usize,
    ) -> std::io::Result<PathBuf> {
        let path = self.home().join(relative_to_home);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, vec![b'x'; size])?;
        Ok(path)
    }
}

impl PlatformService for SandboxPlatform {
    fn platform(&self) -> Platform {
        Platform::current()
    }

    fn known_paths(&self) -> KnownPaths {
        let home = self.home();
        KnownPaths {
            user_cache: Some(home.join("Library/Caches")),
            user_logs: Some(home.join("Library/Logs")),
            app_support: Some(home.join("Library/Application Support")),
            local_app_data: Some(home.join("Library/Application Support")),
            temp: self.temp(),
            trash: Some(home.join(".Trash")),
            downloads: Some(home.join("Downloads")),
            project_roots: vec![home.join("Projects")],
            home,
        }
    }

    fn protected_paths(&self, known: &KnownPaths) -> ProtectedPaths {
        let home = &known.home;
        ProtectedPaths {
            allowed_roots: vec![home.clone(), known.temp.clone()],
            exact: vec![
                home.clone(),
                home.join("Library"),
                home.join("Library/Caches"),
                home.join("Documents"),
            ],
            trees: vec![home.join(".ssh"), home.join("Library/Keychains")],
            app_bundle_roots: vec![home.join("Applications")],
        }
    }
}
