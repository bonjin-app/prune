//! `prune protect`: the list of paths Prune never removes, nor removes anything containing.
//!
//! It edits the same settings file the desktop app does, so a path protected here is shown and
//! honoured there, and the other way round. Two rules keep it from weakening anything by
//! accident:
//!
//! - A change is all or nothing. One bad path in `add` or `remove` and nothing is saved.
//! - While the settings file cannot be read, nothing is changed. What it protects is unknown,
//!   and writing a new file would replace it. The desktop app keeps a copy when it does that;
//!   here the user is told to fix the file or save from the app.

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Subcommand;
use prune_core::settings::{self, Settings};

use crate::{write_json, EXIT_ERROR, EXIT_NOTHING, EXIT_OK};

#[derive(Debug, Subcommand)]
pub enum ProtectAction {
    /// List what is protected. The default.
    List,
    /// Protect these folders or files. Each must exist. `~` means your home folder.
    Add {
        #[arg(value_name = "PATH", required = true)]
        paths: Vec<String>,
    },
    /// Stop protecting these. Works for a path that no longer exists, too.
    Remove {
        #[arg(value_name = "PATH", required = true)]
        paths: Vec<String>,
    },
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProtectedView<'a> {
    protected_paths: &'a [PathBuf],
}

/// Runs `prune protect`. `dir` is the settings folder, `home` what `~` stands for, and `cwd`
/// what a relative path is relative to.
pub fn run(
    dir: Option<&Path>,
    home: Option<&Path>,
    cwd: &Path,
    action: Option<&ProtectAction>,
    json: bool,
    out: &mut impl Write,
) -> std::io::Result<i32> {
    let Some(dir) = dir else {
        writeln!(out, "error: there is no settings folder on this machine")?;
        return Ok(EXIT_ERROR);
    };
    let loaded = Settings::load_checked(dir);
    if let Some(problem) = &loaded.problem {
        writeln!(out, "error: {problem}")?;
        writeln!(
            out,
            "Nothing was changed. Fix the file, or open Settings in the app and save, which \
             keeps a copy of it."
        )?;
        return Ok(EXIT_ERROR);
    }
    let mut settings = loaded.settings;

    let changed = match action.unwrap_or(&ProtectAction::List) {
        ProtectAction::List => false,
        ProtectAction::Add { paths } => {
            let mut added = Vec::new();
            for raw in paths {
                let path = resolve(raw, home, cwd);
                let path = match settings::validate_protected(&path) {
                    Ok(path) => path,
                    Err(reason) => return refuse(out, raw, reason.message()),
                };
                if !settings.protected_paths.contains(&path) && !added.contains(&path) {
                    added.push(path);
                }
            }
            if !json {
                for path in &added {
                    writeln!(out, "Protected {}", path.display())?;
                }
            }
            let changed = !added.is_empty();
            settings.protected_paths.extend(added);
            changed
        }
        ProtectAction::Remove { paths } => {
            let mut removed = Vec::new();
            for raw in paths {
                let Some(path) = stored_entry(&settings.protected_paths, raw, home, cwd) else {
                    return refuse(out, raw, "is not on the list");
                };
                if !removed.contains(&path) {
                    removed.push(path);
                }
            }
            settings.protected_paths.retain(|p| !removed.contains(p));
            if !json {
                for path in &removed {
                    writeln!(out, "No longer protected: {}", path.display())?;
                }
            }
            true
        }
    };

    if changed {
        if let Err(e) = settings.save(dir) {
            writeln!(out, "error: could not save the settings: {e}")?;
            return Ok(EXIT_ERROR);
        }
    }

    if json {
        write_json(
            out,
            &ProtectedView {
                protected_paths: &settings.protected_paths,
            },
        )?;
        return Ok(EXIT_OK);
    }
    if changed {
        writeln!(out)?;
    }
    if settings.protected_paths.is_empty() {
        writeln!(
            out,
            "Nothing is protected. Add a path with `prune protect add <PATH>`."
        )?;
        // Emptying the list on purpose is a success; only asking for an empty one is not.
        return Ok(if changed { EXIT_OK } else { EXIT_NOTHING });
    }
    writeln!(
        out,
        "Protected, with anything inside and anything containing it:"
    )?;
    for path in &settings.protected_paths {
        let missing = std::fs::symlink_metadata(path).is_err();
        writeln!(
            out,
            "  {}{}",
            path.display(),
            if missing { "  (missing now)" } else { "" }
        )?;
    }
    Ok(EXIT_OK)
}

fn refuse(out: &mut impl Write, raw: &str, why: &str) -> std::io::Result<i32> {
    writeln!(out, "error: {raw}: {why}")?;
    writeln!(out, "Nothing was changed.")?;
    Ok(EXIT_ERROR)
}

/// Expands a leading `~` and makes the path absolute. Nothing is resolved on disk here.
///
/// The part after `~` or relative to `cwd` is split on either separator and joined component
/// by component. On Windows `cwd` and the home folder can be `\\?\` paths, where a `/` is part
/// of a name rather than a separator, so joining the text as typed would name another path.
fn resolve(raw: &str, home: Option<&Path>, cwd: &Path) -> PathBuf {
    let raw = raw.trim();
    let (base, rest) = match (raw, home) {
        ("~", Some(home)) => (home, ""),
        (_, Some(home)) if raw.starts_with("~/") || raw.starts_with("~\\") => (home, &raw[2..]),
        _ if Path::new(raw).is_absolute() => return lexically_normal(Path::new(raw)),
        _ => (cwd, raw),
    };
    let mut path = base.to_path_buf();
    for part in rest.split(['/', '\\']) {
        if !part.is_empty() && part != "." {
            path.push(part);
        }
    }
    path
}

/// The list entry `raw` names: written exactly as stored, or the same path in the form `add`
/// stores it, which only works while it exists.
fn stored_entry(stored: &[PathBuf], raw: &str, home: Option<&Path>, cwd: &Path) -> Option<PathBuf> {
    let path = resolve(raw, home, cwd);
    if let Some(found) = stored.iter().find(|p| **p == path) {
        return Some(found.clone());
    }
    let canonical = settings::validate_protected(&path).ok()?;
    stored.iter().find(|p| **p == canonical).cloned()
}

/// Drops `.` components and a trailing separator, so `dist/` and `./dist` match `dist`.
fn lexically_normal(path: &Path) -> PathBuf {
    path.components()
        .filter(|c| !matches!(c, std::path::Component::CurDir))
        .collect()
}

// Unix paths spell the cases plainly; the integration tests cover Windows.
#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn resolve_expands_home_and_relative_paths() {
        let home = Path::new("/home/u");
        let cwd = Path::new("/work");
        assert_eq!(resolve("~", Some(home), cwd), PathBuf::from("/home/u"));
        assert_eq!(
            resolve("~/a/b", Some(home), cwd),
            PathBuf::from("/home/u/a/b")
        );
        assert_eq!(
            resolve("dist", Some(home), cwd),
            PathBuf::from("/work/dist")
        );
        assert_eq!(resolve("/x", Some(home), cwd), PathBuf::from("/x"));
        assert_eq!(
            resolve("~user/x", Some(home), cwd),
            PathBuf::from("/work/~user/x")
        );
    }
}
