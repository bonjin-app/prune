use std::path::Path;

use tauri::State;

use crate::error::{CommandError, CommandResult};
use crate::AppState;

/// Reveal a path in Finder / Explorer. Read-only; the path must be an existing target from a
/// scan (we only check existence + that it lives under an allowed root).
#[tauri::command]
pub fn fs_reveal(state: State<'_, AppState>, path: String) -> CommandResult<()> {
    let p = Path::new(&path);
    if !p.is_absolute() || !p.exists() {
        return Err(CommandError::new("invalid_path", "path does not exist"));
    }
    let engine = state.engine();
    let known = engine.known_paths();
    if !(p.starts_with(&known.home) || p.starts_with(&known.temp) || p.starts_with("/private")) {
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
