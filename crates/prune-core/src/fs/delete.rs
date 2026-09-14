use std::path::Path;

use crate::models::DeleteMode;
use crate::safety::ValidatedPath;
use crate::{PruneError, Result};

/// What happened to a single validated path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoveOutcome {
    MovedToTrash,
    DeletedPermanently,
    /// The path vanished between validation and removal. Not an error.
    AlreadyGone,
}

/// Remove a validated path using the requested mode.
pub fn remove(path: &ValidatedPath, mode: DeleteMode) -> Result<RemoveOutcome> {
    let p = path.as_path();
    let meta = match std::fs::symlink_metadata(p) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(RemoveOutcome::AlreadyGone)
        }
        Err(e) => return Err(PruneError::io(p, e)),
    };

    match mode {
        DeleteMode::Trash => {
            move_to_trash(p)?;
            Ok(RemoveOutcome::MovedToTrash)
        }
        DeleteMode::Permanent => {
            if meta.is_dir() {
                std::fs::remove_dir_all(p).map_err(|e| PruneError::io(p, e))?;
            } else {
                std::fs::remove_file(p).map_err(|e| PruneError::io(p, e))?;
            }
            Ok(RemoveOutcome::DeletedPermanently)
        }
    }
}

#[cfg(target_os = "macos")]
fn move_to_trash(path: &Path) -> Result<()> {
    use trash::macos::{DeleteMethod, TrashContextExtMacos};
    // NSFileManager avoids the Finder/AppleScript route, which is slow and triggers an
    // "Automation" permission prompt.
    let mut ctx = trash::TrashContext::default();
    ctx.set_delete_method(DeleteMethod::NsFileManager);
    ctx.delete(path).map_err(|e| PruneError::Trash {
        path: path.into(),
        message: e.to_string(),
    })
}

#[cfg(not(target_os = "macos"))]
fn move_to_trash(path: &Path) -> Result<()> {
    trash::delete(path).map_err(|e| PruneError::Trash {
        path: path.into(),
        message: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::ProtectedPaths;
    use crate::safety::SafetyPolicy;

    #[test]
    fn permanent_delete_removes_tree() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        std::fs::create_dir_all(home.join("cache/nested")).unwrap();
        std::fs::write(home.join("cache/nested/f"), b"x").unwrap();
        let policy = SafetyPolicy::from_protected(ProtectedPaths {
            allowed_roots: vec![home.clone()],
            ..Default::default()
        });
        let v = policy.validate(&home.join("cache")).unwrap();
        assert_eq!(
            remove(&v, DeleteMode::Permanent).unwrap(),
            RemoveOutcome::DeletedPermanently
        );
        assert!(!home.join("cache").exists());
        assert_eq!(
            remove(&v, DeleteMode::Permanent).unwrap(),
            RemoveOutcome::AlreadyGone
        );
    }
}
