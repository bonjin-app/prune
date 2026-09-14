use std::path::{Component, Path, PathBuf};

use crate::platform::{KnownPaths, PlatformService, ProtectedPaths};
use crate::{PruneError, Result};

/// A path that passed validation. Only this type can be handed to the deletion functions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPath {
    normalized: PathBuf,
}

impl ValidatedPath {
    pub fn as_path(&self) -> &Path {
        &self.normalized
    }
}

/// Validates paths against protected locations for the current platform.
#[derive(Debug, Clone)]
pub struct SafetyPolicy {
    allowed_roots: Vec<PathBuf>,
    exact: Vec<PathBuf>,
    trees: Vec<PathBuf>,
}

impl SafetyPolicy {
    /// Policy for the running OS.
    pub fn for_platform(platform: &dyn PlatformService, known: &KnownPaths) -> Self {
        Self::from_protected(platform.protected_paths(known))
    }

    /// Build a policy from explicit lists (used by tests and by the sandbox).
    pub fn from_protected(protected: ProtectedPaths) -> Self {
        let norm = |v: Vec<PathBuf>| v.into_iter().map(|p| canonical_or_lexical(&p)).collect();
        Self {
            allowed_roots: norm(protected.allowed_roots),
            exact: norm(protected.exact),
            trees: norm(protected.trees),
        }
    }

    /// Validate `path` for removal.
    ///
    /// Rules, in order:
    /// 1. must be absolute and free of `.`/`..` components
    /// 2. must exist (symlinks are *not* followed; a symlink is removed as a link)
    /// 3. must be inside one of the allowed roots, and not *be* an allowed root
    /// 4. must not be an exact-protected path or an ancestor of one
    /// 5. must not be inside a protected tree
    pub fn validate(&self, path: &Path) -> Result<ValidatedPath> {
        if !path.is_absolute() {
            return Err(PruneError::safety(path, "path must be absolute"));
        }
        if path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        {
            return Err(PruneError::safety(
                path,
                "path must not contain '.' or '..'",
            ));
        }
        if path.parent().is_none() {
            return Err(PruneError::safety(
                path,
                "refusing to touch a filesystem root",
            ));
        }

        let meta = std::fs::symlink_metadata(path)
            .map_err(|_| PruneError::safety(path, "path does not exist"))?;
        let _ = meta;

        let normalized = normalize_existing(path)?;

        let inside_allowed = self
            .allowed_roots
            .iter()
            .any(|root| normalized.starts_with(root) && normalized != *root);
        if !inside_allowed {
            return Err(PruneError::safety(
                path,
                "outside of the directories Prune is allowed to modify (home, temp)",
            ));
        }

        for exact in &self.exact {
            if normalized == *exact {
                return Err(PruneError::safety(
                    path,
                    "protected system or user directory",
                ));
            }
            if exact.starts_with(&normalized) {
                return Err(PruneError::safety(path, "contains a protected directory"));
            }
        }

        for tree in &self.trees {
            if normalized.starts_with(tree) {
                return Err(PruneError::safety(path, "inside a protected tree"));
            }
        }

        Ok(ValidatedPath { normalized })
    }

    /// `true` when the path would be refused. Convenience for risk classification.
    pub fn is_protected(&self, path: &Path) -> bool {
        self.validate(path).is_err()
    }
}

/// Canonicalize the parent (which must exist) and re-attach the final component so that a
/// symlink at the leaf is not followed. Falls back to lexical normalization.
fn normalize_existing(path: &Path) -> Result<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| PruneError::safety(path, "no parent directory"))?;
    let name = path
        .file_name()
        .ok_or_else(|| PruneError::safety(path, "no file name"))?;
    let parent = std::fs::canonicalize(parent).map_err(|e| PruneError::io(parent, e))?;
    Ok(strip_verbatim(parent.join(name)))
}

fn canonical_or_lexical(path: &Path) -> PathBuf {
    match std::fs::canonicalize(path) {
        Ok(p) => strip_verbatim(p),
        Err(_) => path.to_path_buf(),
    }
}

/// On Windows `canonicalize` yields `\\?\C:\…`. Strip the verbatim prefix so comparisons with
/// plain paths (from env vars) work.
fn strip_verbatim(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let s = path.to_string_lossy();
        if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = s.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
        path
    }
    #[cfg(not(windows))]
    {
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox() -> (tempfile::TempDir, SafetyPolicy) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join("home/Library/Caches/app")).unwrap();
        std::fs::create_dir_all(root.join("home/.ssh")).unwrap();
        std::fs::create_dir_all(root.join("home/Documents/important")).unwrap();
        std::fs::create_dir_all(root.join("system/bin")).unwrap();
        std::fs::write(root.join("home/.ssh/id_ed25519"), b"secret").unwrap();
        std::fs::write(root.join("home/Library/Caches/app/blob"), b"cache").unwrap();
        std::fs::write(root.join("home/Documents/important/thesis.txt"), b"data").unwrap();

        let home = root.join("home");
        let policy = SafetyPolicy::from_protected(ProtectedPaths {
            allowed_roots: vec![home.clone()],
            exact: vec![home.clone(), home.join("Library"), home.join("Documents")],
            trees: vec![home.join(".ssh"), root.join("system")],
        });
        (dir, policy)
    }

    #[test]
    fn accepts_cache_inside_home() {
        let (dir, policy) = sandbox();
        let target = dir.path().join("home/Library/Caches/app");
        assert!(policy.validate(&target).is_ok());
    }

    #[test]
    fn rejects_relative_and_dotdot() {
        let (_dir, policy) = sandbox();
        assert!(policy.validate(Path::new("relative/path")).is_err());
        let with_dots = std::env::temp_dir().join("..").join("x");
        assert!(policy.validate(&with_dots).is_err());
    }

    #[test]
    fn rejects_root_and_home_itself() {
        let (dir, policy) = sandbox();
        assert!(policy.validate(Path::new("/")).is_err());
        assert!(policy.validate(&dir.path().join("home")).is_err());
    }

    #[test]
    fn rejects_exact_protected_and_ancestors() {
        let (dir, policy) = sandbox();
        assert!(policy.validate(&dir.path().join("home/Library")).is_err());
        assert!(policy.validate(&dir.path().join("home/Documents")).is_err());
        // Children of exact-protected dirs are allowed.
        assert!(policy
            .validate(&dir.path().join("home/Documents/important"))
            .is_ok());
    }

    #[test]
    fn rejects_protected_trees() {
        let (dir, policy) = sandbox();
        assert!(policy.validate(&dir.path().join("home/.ssh")).is_err());
        assert!(policy
            .validate(&dir.path().join("home/.ssh/id_ed25519"))
            .is_err());
        assert!(policy.validate(&dir.path().join("system/bin")).is_err());
    }

    #[test]
    fn rejects_outside_allowed_roots() {
        let (dir, policy) = sandbox();
        std::fs::create_dir_all(dir.path().join("elsewhere/x")).unwrap();
        assert!(policy.validate(&dir.path().join("elsewhere/x")).is_err());
    }

    #[test]
    fn rejects_missing_path() {
        let (dir, policy) = sandbox();
        assert!(policy
            .validate(&dir.path().join("home/Library/Caches/nope"))
            .is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_leaf_is_not_followed_into_protected_tree() {
        let (dir, policy) = sandbox();
        let link = dir.path().join("home/Library/Caches/sneaky");
        std::os::unix::fs::symlink(dir.path().join("home/.ssh"), &link).unwrap();
        // Removing the *link* is fine; the link itself lives in an allowed place.
        let validated = policy.validate(&link).unwrap();
        let expected = std::fs::canonicalize(link.parent().unwrap())
            .unwrap()
            .join("sneaky");
        assert_eq!(validated.as_path(), expected.as_path());
        // But paths *through* the link resolve into the protected tree and are refused.
        assert!(policy.validate(&link.join("id_ed25519")).is_err());
    }
}
