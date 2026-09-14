//! launchd agents and daemons.
//!
//! Prune reads the `.plist` files and asks `launchctl` for the per-user override database,
//! which is what actually decides whether an agent loads. Toggling writes only to that
//! override database (`launchctl enable|disable`), so nothing is deleted or edited and the
//! change is reversible from Prune or the terminal.
//!
//! Applications registered through macOS's own Login Items API (`SMAppService`) live in a
//! SIP-protected database that cannot be read without elevated rights; those stay in
//! System Settings and are deliberately not reported here.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::models::CleanupTarget;
use crate::platform::{KnownPaths, StartupItem, StartupScope, StartupTrigger};
use crate::{PruneError, Result};

struct PlistInfo {
    label: Option<String>,
    command: Option<String>,
    trigger: StartupTrigger,
    disabled_in_plist: bool,
}

fn read_plist(path: &Path) -> PlistInfo {
    let none = PlistInfo {
        label: None,
        command: None,
        trigger: StartupTrigger::OnDemand,
        disabled_in_plist: false,
    };
    let Ok(value) = plist::Value::from_file(path) else {
        return none;
    };
    let Some(dict) = value.as_dictionary() else {
        return none;
    };

    let label = dict
        .get("Label")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());
    let command = dict
        .get("Program")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string())
        .or_else(|| {
            dict.get("ProgramArguments")
                .and_then(|v| v.as_array())
                .map(|args| {
                    args.iter()
                        .filter_map(|a| a.as_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .filter(|s| !s.is_empty())
        });
    let run_at_load = dict
        .get("RunAtLoad")
        .and_then(|v| v.as_boolean())
        .unwrap_or(false);
    let keep_alive = dict
        .get("KeepAlive")
        .is_some_and(|v| v.as_boolean() != Some(false));
    let scheduled =
        dict.contains_key("StartInterval") || dict.contains_key("StartCalendarInterval");
    let trigger = if keep_alive {
        StartupTrigger::KeepAlive
    } else if run_at_load {
        StartupTrigger::AtLogin
    } else if scheduled {
        StartupTrigger::Scheduled
    } else {
        StartupTrigger::OnDemand
    };
    PlistInfo {
        label,
        command,
        trigger,
        disabled_in_plist: dict
            .get("Disabled")
            .and_then(|v| v.as_boolean())
            .unwrap_or(false),
    }
}

/// Current user's numeric id, read from the ownership of the home directory so no extra
/// dependency is needed.
fn uid(known: &KnownPaths) -> Option<u32> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(&known.home).ok().map(|m| m.uid())
}

/// Labels the launchd override database marks as disabled.
///
/// `launchctl print-disabled gui/<uid>` prints lines like `"com.foo.bar" => disabled`
/// (older systems print `=> true`).
fn disabled_labels(uid: u32) -> Vec<String> {
    let Ok(out) = Command::new("launchctl")
        .args(["print-disabled", &format!("gui/{uid}")])
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let (label, state) = line.split_once("=>")?;
            let state = state.trim().trim_end_matches(',').to_ascii_lowercase();
            if state != "disabled" && state != "true" {
                return None;
            }
            Some(label.trim().trim_matches('"').to_string())
        })
        .collect()
}

fn pretty_name(label: &str, path: &Path) -> String {
    // `com.google.keystone.agent` → `Google Keystone Agent`
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let base = if label.is_empty() {
        stem.as_str()
    } else {
        label
    };
    let parts: Vec<&str> = base.split('.').collect();
    let tail = if parts.len() > 2 {
        &parts[2..]
    } else {
        &parts[..]
    };
    let words: Vec<String> = tail
        .iter()
        .flat_map(|p| p.split(['-', '_']))
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect();
    if words.is_empty() {
        base.to_string()
    } else {
        words.join(" ")
    }
}

fn scan(
    dir: &Path,
    source: &str,
    scope: StartupScope,
    disabled: &[String],
    out: &mut Vec<StartupItem>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "plist"))
        .collect();
    paths.sort();
    for path in paths {
        let info = read_plist(&path);
        let label = info.label.clone().unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        });
        let overridden = disabled.contains(&label);
        let can_toggle = scope == StartupScope::User;
        out.push(StartupItem {
            id: CleanupTarget::make_id("startup", &path),
            name: pretty_name(&label, &path),
            label: Some(label),
            path: path.to_string_lossy().into_owned(),
            command: info.command,
            enabled: !(overridden || info.disabled_in_plist),
            source: source.to_string(),
            scope,
            trigger: info.trigger,
            can_toggle,
            reason: (!can_toggle).then(|| {
                "Installed for all users; change it with administrator rights.".to_string()
            }),
        });
    }
}

pub fn list(known: &KnownPaths) -> Vec<StartupItem> {
    let disabled = uid(known).map(disabled_labels).unwrap_or_default();
    let mut out = Vec::new();
    scan(
        &known.home.join("Library/LaunchAgents"),
        "launch_agent",
        StartupScope::User,
        &disabled,
        &mut out,
    );
    scan(
        Path::new("/Library/LaunchAgents"),
        "launch_agent",
        StartupScope::System,
        &disabled,
        &mut out,
    );
    scan(
        Path::new("/Library/LaunchDaemons"),
        "launch_daemon",
        StartupScope::System,
        &disabled,
        &mut out,
    );
    out.sort_by(|a, b| {
        b.enabled
            .cmp(&a.enabled)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out
}

pub fn set_enabled(item: &StartupItem, enabled: bool) -> Result<()> {
    if !item.can_toggle {
        return Err(PruneError::Other(
            item.reason
                .clone()
                .unwrap_or_else(|| "this item cannot be changed".into()),
        ));
    }
    let label = item
        .label
        .as_deref()
        .ok_or_else(|| PruneError::Other("item has no launchd label".into()))?;
    let uid = std::fs::metadata(
        Path::new(&item.path)
            .ancestors()
            .find(|p| p.ends_with("Library"))
            .unwrap_or(Path::new(&item.path)),
    )
    .map(|m| {
        use std::os::unix::fs::MetadataExt;
        m.uid()
    })
    .map_err(|e| PruneError::io(&item.path, e))?;

    let verb = if enabled { "enable" } else { "disable" };
    let out = Command::new("launchctl")
        .args([verb, &format!("gui/{uid}/{label}")])
        .output()
        .map_err(|e| PruneError::io("launchctl", e))?;
    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(PruneError::Other(if msg.is_empty() {
            format!("launchctl {verb} failed")
        } else {
            msg
        }));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plist_fields_and_trigger() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("com.example.helper.plist");
        std::fs::write(
            &path,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>com.example.helper</string>
<key>ProgramArguments</key><array><string>/usr/local/bin/helper</string><string>--serve</string></array>
<key>RunAtLoad</key><true/>
</dict></plist>"#,
        )
        .unwrap();
        let info = read_plist(&path);
        assert_eq!(info.label.as_deref(), Some("com.example.helper"));
        assert_eq!(
            info.command.as_deref(),
            Some("/usr/local/bin/helper --serve")
        );
        assert_eq!(info.trigger, StartupTrigger::AtLogin);
        assert!(!info.disabled_in_plist);
    }

    #[test]
    fn scan_marks_overridden_items_disabled() {
        let dir = tempfile::tempdir().unwrap();
        for (name, label) in [("a.plist", "com.example.a"), ("b.plist", "com.example.b")] {
            std::fs::write(
                dir.path().join(name),
                format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>Label</key><string>{label}</string>
<key>KeepAlive</key><true/></dict></plist>"#
                ),
            )
            .unwrap();
        }
        let mut out = Vec::new();
        scan(
            dir.path(),
            "launch_agent",
            StartupScope::User,
            &["com.example.b".to_string()],
            &mut out,
        );
        assert_eq!(out.len(), 2);
        let a = out
            .iter()
            .find(|i| i.label.as_deref() == Some("com.example.a"))
            .unwrap();
        let b = out
            .iter()
            .find(|i| i.label.as_deref() == Some("com.example.b"))
            .unwrap();
        assert!(a.enabled);
        assert!(!b.enabled);
        assert!(a.can_toggle);
        assert_eq!(a.trigger, StartupTrigger::KeepAlive);
    }

    #[test]
    fn system_scope_is_read_only() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("x.plist"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>Label</key><string>com.vendor.daemon</string></dict></plist>"#,
        )
        .unwrap();
        let mut out = Vec::new();
        scan(
            dir.path(),
            "launch_daemon",
            StartupScope::System,
            &[],
            &mut out,
        );
        assert!(!out[0].can_toggle);
        assert!(out[0].reason.is_some());
        assert!(set_enabled(&out[0], false).is_err());
    }

    #[test]
    fn pretty_name_drops_reverse_dns_prefix() {
        assert_eq!(
            pretty_name(
                "com.google.keystone.agent",
                Path::new("/x/com.google.keystone.agent.plist")
            ),
            "Keystone Agent"
        );
        assert_eq!(
            pretty_name("", Path::new("/x/my-helper.plist")),
            "My Helper"
        );
    }
}
