use std::path::Path;

use tauri::State;

use crate::error::{CommandError, CommandResult};
use crate::AppState;

/// Reveals a path in Finder or Explorer.
///
/// Opening a window is harmless, but the path still has to be one Prune would work with,
/// otherwise the command becomes a way to point the file manager anywhere on the machine.
/// Everything is compared in canonical form: on macOS the temp directory is reached both as
/// `/var/folders/…` and `/private/var/folders/…`, and accepting the `/private` prefix wholesale
/// would also accept `/private/etc`.
#[tauri::command]
pub fn fs_reveal(state: State<'_, AppState>, path: String) -> CommandResult<()> {
    let p = Path::new(&path);
    if !p.is_absolute() || !p.exists() {
        return Err(CommandError::new("invalid_path", "path does not exist"));
    }
    let engine = state.engine();
    if !engine.policy().is_inside_allowed_root(p) {
        return Err(CommandError::new(
            "invalid_path",
            "path is outside the user's directories",
        ));
    }
    reveal(p)
}

#[cfg(target_os = "macos")]
fn reveal(p: &Path) -> CommandResult<()> {
    std::process::Command::new("open")
        .arg("-R")
        .arg(p)
        .spawn()?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn reveal(p: &Path) -> CommandResult<()> {
    std::process::Command::new("explorer")
        .arg(format!("/select,{}", p.display()))
        .spawn()?;
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn reveal(p: &Path) -> CommandResult<()> {
    let dir = if p.is_dir() {
        p
    } else {
        p.parent().unwrap_or(p)
    };
    std::process::Command::new("xdg-open").arg(dir).spawn()?;
    Ok(())
}
