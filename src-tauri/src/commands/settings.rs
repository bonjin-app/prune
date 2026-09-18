use std::path::PathBuf;

use prune_core::settings::{self, Settings};
use prune_core::sync::LockExt;
use serde::Serialize;
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
}

fn view(state: &AppState) -> SettingsView {
    let engine = state.engine();
    let settings = state.settings.lock_recover().clone();
    SettingsView {
        project_roots_configured: engine.project_roots_are_configured(&settings),
        effective_project_roots: engine
            .known_paths()
            .project_roots
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
        home_dir: engine.known_paths().home.to_string_lossy().into_owned(),
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
    state
        .apply_settings(Settings {
            project_roots: validated,
        })
        .map_err(CommandError::from)?;
    Ok(view(&state))
}
