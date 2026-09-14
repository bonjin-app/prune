//! Windows startup entries: the `Run` registry keys and the Start Menu `Startup` folders.
//!
//! Enabling and disabling writes only to `StartupApproved`, the same per-user database Task
//! Manager uses. The original `Run` value or shortcut is never touched, so the change is
//! reversible and no program is uninstalled.

use std::path::{Path, PathBuf};

use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE};
use winreg::RegKey;

use crate::models::CleanupTarget;
use crate::platform::{startup_approved, KnownPaths, StartupItem, StartupScope, StartupTrigger};
use crate::{PruneError, Result};

const RUN: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";
const RUN_WOW: &str = r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run";
const APPROVED_RUN: &str =
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
const APPROVED_FOLDER: &str =
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\StartupFolder";

/// Reads one entry's state from `StartupApproved`. The byte format lives in
/// [`startup_approved`], which is unit-tested on every platform.
fn approved_state(hive: winreg::HKEY, subkey: &str, name: &str) -> Option<bool> {
    let key = RegKey::predef(hive)
        .open_subkey_with_flags(subkey, KEY_READ)
        .ok()?;
    let value = key.get_raw_value(name).ok()?;
    startup_approved::is_enabled(&value.bytes)
}

fn write_approved(subkey: &str, name: &str, enabled: bool) -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey_with_flags(subkey, KEY_READ | KEY_WRITE)
        .map_err(|e| PruneError::Other(format!("cannot open {subkey}: {e}")))?;
    let existing = key.get_raw_value(name).map(|v| v.bytes).ok();
    let bytes = startup_approved::with_state(existing.as_deref(), enabled);
    let value = winreg::RegValue {
        bytes,
        vtype: winreg::enums::RegType::REG_BINARY,
    };
    key.set_raw_value(name, &value)
        .map_err(|e| PruneError::Other(format!("cannot write {subkey}\\{name}: {e}")))
}

fn scan_run_key(hive: winreg::HKEY, subkey: &str, scope: StartupScope, out: &mut Vec<StartupItem>) {
    let Ok(key) = RegKey::predef(hive).open_subkey_with_flags(subkey, KEY_READ) else {
        return;
    };
    for (name, value) in key.enum_values().filter_map(|v| v.ok()) {
        let command = value.to_string();
        let enabled = approved_state(hive, APPROVED_RUN, &name)
            .or_else(|| approved_state(HKEY_CURRENT_USER, APPROVED_RUN, &name))
            .unwrap_or(true);
        let can_toggle = scope == StartupScope::User;
        let full = format!("{subkey}\\{name}");
        out.push(StartupItem {
            id: CleanupTarget::make_id("startup", Path::new(&full)),
            name: name.clone(),
            label: Some(name),
            path: full,
            command: Some(command).filter(|c| !c.is_empty()),
            enabled,
            source: "registry_run".into(),
            scope,
            trigger: StartupTrigger::AtLogin,
            can_toggle,
            reason: (!can_toggle).then(|| {
                "Installed for all users; change it with administrator rights.".to_string()
            }),
        });
    }
}

fn scan_startup_folder(dir: &Path, scope: StartupScope, out: &mut Vec<StartupItem>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && !p.file_name().is_some_and(|n| n == "desktop.ini"))
        .collect();
    paths.sort();
    for path in paths {
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let name = path
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| file_name.clone());
        let enabled =
            approved_state(HKEY_CURRENT_USER, APPROVED_FOLDER, &file_name).unwrap_or(true);
        let can_toggle = scope == StartupScope::User;
        out.push(StartupItem {
            id: CleanupTarget::make_id("startup", &path),
            name,
            label: Some(file_name),
            path: path.to_string_lossy().into_owned(),
            command: None,
            enabled,
            source: "startup_folder".into(),
            scope,
            trigger: StartupTrigger::AtLogin,
            can_toggle,
            reason: (!can_toggle)
                .then(|| "Shared by all users; change it with administrator rights.".to_string()),
        });
    }
}

pub fn list(known: &KnownPaths) -> Result<Vec<StartupItem>> {
    let mut out = Vec::new();
    scan_run_key(HKEY_CURRENT_USER, RUN, StartupScope::User, &mut out);
    scan_run_key(HKEY_LOCAL_MACHINE, RUN, StartupScope::System, &mut out);
    scan_run_key(HKEY_LOCAL_MACHINE, RUN_WOW, StartupScope::System, &mut out);

    if let Some(roaming) = &known.app_support {
        scan_startup_folder(
            &roaming.join(r"Microsoft\Windows\Start Menu\Programs\Startup"),
            StartupScope::User,
            &mut out,
        );
    }
    if let Some(program_data) = std::env::var_os("ProgramData").map(PathBuf::from) {
        scan_startup_folder(
            &program_data.join(r"Microsoft\Windows\Start Menu\Programs\StartUp"),
            StartupScope::System,
            &mut out,
        );
    }
    out.sort_by(|a, b| {
        b.enabled
            .cmp(&a.enabled)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(out)
}

pub fn set_enabled(item: &StartupItem, enabled: bool) -> Result<()> {
    if !item.can_toggle {
        return Err(PruneError::Other(
            item.reason
                .clone()
                .unwrap_or_else(|| "this item cannot be changed".into()),
        ));
    }
    let name = item
        .label
        .as_deref()
        .ok_or_else(|| PruneError::Other("item has no registry value name".into()))?;
    let subkey = match item.source.as_str() {
        "registry_run" => APPROVED_RUN,
        "startup_folder" => APPROVED_FOLDER,
        other => {
            return Err(PruneError::Other(format!(
                "unsupported startup source: {other}"
            )))
        }
    };
    write_approved(subkey, name, enabled)
}
