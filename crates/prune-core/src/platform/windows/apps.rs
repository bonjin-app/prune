//! Application discovery on Windows via the `Uninstall` registry keys, plus AppData folders
//! named after the application.

use std::path::PathBuf;

use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
use winreg::RegKey;

use super::{AppDataKind, ApplicationInfo, KnownPaths, RelatedPath};
use crate::models::CleanupTarget;
use crate::Result;

const UNINSTALL_KEYS: &[(winreg::HKEY, &str, &str)] = &[
    (
        HKEY_LOCAL_MACHINE,
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        "registry",
    ),
    (
        HKEY_LOCAL_MACHINE,
        r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        "registry",
    ),
    (
        HKEY_CURRENT_USER,
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        "registry_user",
    ),
];

pub fn list(_known: &KnownPaths) -> Result<Vec<ApplicationInfo>> {
    let mut out: Vec<ApplicationInfo> = Vec::new();
    for (hive, path, source) in UNINSTALL_KEYS {
        let Ok(root) = RegKey::predef(*hive).open_subkey_with_flags(path, KEY_READ) else {
            continue;
        };
        for key_name in root.enum_keys().filter_map(|k| k.ok()) {
            let Ok(key) = root.open_subkey_with_flags(&key_name, KEY_READ) else {
                continue;
            };
            let get = |name: &str| {
                key.get_value::<String, _>(name)
                    .ok()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            };
            let Some(name) = get("DisplayName") else {
                continue;
            };
            let system_component: u32 = key.get_value("SystemComponent").unwrap_or(0);
            if system_component == 1 {
                continue;
            }
            let publisher = get("Publisher");
            let is_system = publisher
                .as_deref()
                .is_some_and(|p| p.starts_with("Microsoft"))
                && name.starts_with("Microsoft");
            let size_kb: Option<u32> = key.get_value("EstimatedSize").ok();
            let install_location = get("InstallLocation").unwrap_or_default();
            let full_key = format!("{path}\\{key_name}");
            out.push(ApplicationInfo {
                id: CleanupTarget::make_id("app", std::path::Path::new(&full_key)),
                name,
                path: install_location,
                version: get("DisplayVersion"),
                bundle_id: Some(key_name.clone()),
                publisher,
                size_bytes: size_kb.map(|kb| u64::from(kb) * 1024),
                modified_at: None,
                source: source.to_string(),
                is_system,
                uninstall_command: get("QuietUninstallString").or_else(|| get("UninstallString")),
            });
        }
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    out.dedup_by(|a, b| a.name == b.name && a.version == b.version);
    Ok(out)
}

pub fn related_paths(app: &ApplicationInfo, known: &KnownPaths) -> Vec<RelatedPath> {
    let mut out: Vec<RelatedPath> = Vec::new();
    let mut push = |kind: AppDataKind, path: PathBuf| {
        if path.exists() && !out.iter().any(|r| r.path == path) {
            out.push(RelatedPath { kind, path });
        }
    };
    if !app.path.is_empty() {
        let p = PathBuf::from(&app.path);
        // Install locations under the user profile can be removed; Program Files cannot
        // (the vendor uninstaller handles those) and would be marked Protected anyway.
        push(AppDataKind::Application, p);
    }
    let name = app.name.trim();
    if name.len() >= 3 {
        if let Some(local) = &known.local_app_data {
            push(AppDataKind::LocalAppData, local.join(name));
        }
        if let Some(roaming) = &known.app_support {
            push(AppDataKind::RoamingAppData, roaming.join(name));
        }
        if let Some(publisher) = app.publisher.as_deref() {
            if let Some(local) = &known.local_app_data {
                push(AppDataKind::LocalAppData, local.join(publisher).join(name));
            }
            if let Some(roaming) = &known.app_support {
                push(
                    AppDataKind::RoamingAppData,
                    roaming.join(publisher).join(name),
                );
            }
        }
    }
    out
}
