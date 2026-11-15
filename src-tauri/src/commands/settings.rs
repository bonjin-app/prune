use std::path::PathBuf;

use prune_core::settings::{self, Settings};
use prune_core::sync::LockExt;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{CommandError, CommandResult};
use crate::AppState;

/// The settings plus what they currently resolve to, so the UI never has to guess.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub settings: Settings,
    /// The directories that will actually be searched for project artifacts.
    pub effective_project_roots: Vec<String>,
    /// `false` when Prune is guessing because nothing has been configured.
    pub project_roots_configured: bool,
    pub home_dir: String,
    /// Why the settings file could not be read, while nothing may be removed because of it.
    pub problem: Option<String>,
}

fn view(state: &AppState) -> SettingsView {
    let engine = state.engine();
    let settings = state.current_settings();
    SettingsView {
        project_roots_configured: engine.project_roots_are_configured(&settings),
        effective_project_roots: engine
            .known_paths()
            .project_roots
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
        home_dir: engine.known_paths().home.to_string_lossy().into_owned(),
        problem: state.settings_problem.lock_recover().clone(),
        settings,
    }
}

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> CommandResult<SettingsView> {
    Ok(view(&state))
}

/// Replaces the list of folders searched for project artifacts.
///
/// Each folder is validated before anything is saved, so a typo cannot silently disable the
/// developer scan. An empty list restores the guessed defaults.
#[tauri::command]
pub fn settings_set_project_roots(
    state: State<'_, AppState>,
    roots: Vec<String>,
) -> CommandResult<SettingsView> {
    let home = state.engine().known_paths().home.clone();
    let mut validated: Vec<PathBuf> = Vec::new();
    for raw in &roots {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        match settings::validate_root(&PathBuf::from(trimmed), &home) {
            Ok(path) => validated.push(path),
            Err(reason) => {
                return Err(CommandError::new(
                    "invalid_path",
                    format!("{trimmed}: {}", reason.message()),
                ))
            }
        }
    }
    // Only the project folders change; everything else, protections included, is kept.
    let mut settings = state.current_settings();
    settings.project_roots = validated;
    state.apply_settings(settings).map_err(CommandError::from)?;
    Ok(view(&state))
}

/// Replaces the list of paths Prune must never remove, nor remove anything containing.
///
/// A path new to the list must exist, so a typo cannot pass for a protection. One already on it
/// is kept as it is even if it has gone missing, so it can still be removed from the list — and
/// stays protected until it is.
#[tauri::command]
pub fn settings_set_protected_paths(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> CommandResult<SettingsView> {
    let mut settings = state.current_settings();
    let mut validated: Vec<PathBuf> = Vec::new();
    for raw in &paths {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let path = PathBuf::from(trimmed);
        let path = if settings.protected_paths.contains(&path) {
            path
        } else {
            settings::validate_protected(&path).map_err(|reason| {
                CommandError::new("invalid_path", format!("{trimmed}: {}", reason.message()))
            })?
        };
        if !validated.contains(&path) {
            validated.push(path);
        }
    }
    settings.protected_paths = validated;
    state.apply_settings(settings).map_err(CommandError::from)?;
    Ok(view(&state))
}

/// What the native panel lets the user choose.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PickKind {
    Folder,
    File,
}

/// Opens the system's own panel for choosing a folder or file to protect, starting in the home
/// folder. Returns the chosen path, or nothing if the panel was cancelled.
///
/// Choosing saves nothing: the path still goes through `settings_set_protected_paths` and its
/// checks. The panel is opened here rather than from the page so the webview is granted no
/// dialog permissions of its own.
#[tauri::command]
pub async fn settings_pick_protected_path<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    kind: PickKind,
) -> CommandResult<Option<String>> {
    use tauri_plugin_dialog::DialogExt;

    let mut panel = app.dialog().file().set_title(match kind {
        PickKind::Folder => "Choose a folder to protect",
        PickKind::File => "Choose a file to protect",
    });
    if let Some(home) = dirs_home(&app) {
        panel = panel.set_directory(home);
    }
    // The blocking pickers must not run on the thread that drives the window.
    let picked = tauri::async_runtime::spawn_blocking(move || match kind {
        PickKind::Folder => panel.blocking_pick_folder(),
        PickKind::File => panel.blocking_pick_file(),
    })
    .await
    .map_err(|e| CommandError::new("other", e.to_string()))?;
    Ok(picked
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned()))
}

fn dirs_home<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Option<PathBuf> {
    use tauri::Manager;
    app.path().home_dir().ok()
}
