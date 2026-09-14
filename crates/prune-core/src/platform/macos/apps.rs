//! Application discovery on macOS: `.app` bundles in `/Applications` and `~/Applications`,
//! plus the per-user data a bundle identifier leaves behind under `~/Library`.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

use super::{AppDataKind, ApplicationInfo, KnownPaths, RelatedPath};
use crate::models::CleanupTarget;

struct BundleMeta {
    name: Option<String>,
    bundle_id: Option<String>,
    version: Option<String>,
}

fn read_info_plist(bundle: &Path) -> BundleMeta {
    let info = bundle.join("Contents/Info.plist");
    let Ok(value) = plist::Value::from_file(&info) else {
        return BundleMeta {
            name: None,
            bundle_id: None,
            version: None,
        };
    };
    let dict = value.as_dictionary();
    let get = |key: &str| {
        dict.and_then(|d| d.get(key))
            .and_then(|v| v.as_string())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    BundleMeta {
        name: get("CFBundleDisplayName").or_else(|| get("CFBundleName")),
        bundle_id: get("CFBundleIdentifier"),
        version: get("CFBundleShortVersionString").or_else(|| get("CFBundleVersion")),
    }
}

fn scan_dir(dir: &Path, source: &str, out: &mut Vec<ApplicationInfo>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let is_app = path.extension().is_some_and(|e| e == "app");
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if !meta.is_dir() {
            continue;
        }
        if !is_app {
            // One level of folders (e.g. /Applications/Utilities, vendor folders).
            if source == "applications" && path.file_name().is_some_and(|n| n != "Utilities") {
                scan_dir(&path, "applications", out);
            }
            continue;
        }
        let bm = read_info_plist(&path);
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let is_system = bm
            .bundle_id
            .as_deref()
            .is_some_and(|b| b.starts_with("com.apple."))
            || path.starts_with("/System");
        out.push(ApplicationInfo {
            id: CleanupTarget::make_id("app", &path),
            // Finder shows the bundle file name; CFBundleName is often a short internal name
            // ("Code" for Visual Studio Code), so prefer the file stem.
            name: if stem.is_empty() {
                bm.name.unwrap_or_default()
            } else {
                stem
            },
            path: path.to_string_lossy().into_owned(),
            version: bm.version,
            bundle_id: bm.bundle_id,
            publisher: None,
            size_bytes: None,
            modified_at: meta.modified().ok().map(DateTime::<Utc>::from),
            source: source.to_string(),
            is_system,
            uninstall_command: None,
        });
    }
}

pub fn list(known: &KnownPaths) -> Vec<ApplicationInfo> {
    let mut out = Vec::new();
    scan_dir(Path::new("/Applications"), "applications", &mut out);
    scan_dir(
        &known.home.join("Applications"),
        "user_applications",
        &mut out,
    );
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

/// Matches `<dir>/<bundle_id>*` entries (e.g. `com.foo.Bar.plist`, `com.foo.Bar.savedState`).
fn prefixed_children(dir: &Path, prefix: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![];
    };
    entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                n == prefix
                    || n.strip_prefix(prefix)
                        .is_some_and(|rest| rest.starts_with('.') || rest.starts_with('-'))
            })
        })
        .collect()
}

pub fn related_paths(app: &ApplicationInfo, known: &KnownPaths) -> Vec<RelatedPath> {
    let lib = known.home.join("Library");
    let mut out: Vec<RelatedPath> = Vec::new();
    let mut push = |kind: AppDataKind, path: PathBuf| {
        if path.exists() && !out.iter().any(|r| r.path == path) {
            out.push(RelatedPath { kind, path });
        }
    };

    push(AppDataKind::Application, PathBuf::from(&app.path));

    if let Some(bid) = app.bundle_id.as_deref() {
        push(AppDataKind::Caches, lib.join("Caches").join(bid));
        push(
            AppDataKind::ApplicationSupport,
            lib.join("Application Support").join(bid),
        );
        push(AppDataKind::Containers, lib.join("Containers").join(bid));
        push(AppDataKind::WebKit, lib.join("WebKit").join(bid));
        push(
            AppDataKind::HttpStorages,
            lib.join("HTTPStorages").join(bid),
        );
        push(
            AppDataKind::ApplicationScripts,
            lib.join("Application Scripts").join(bid),
        );
        push(AppDataKind::Logs, lib.join("Logs").join(bid));
        for p in prefixed_children(&lib.join("Preferences"), bid) {
            push(AppDataKind::Preferences, p);
        }
        for p in prefixed_children(&lib.join("Saved Application State"), bid) {
            push(AppDataKind::SavedState, p);
        }
        for p in prefixed_children(&lib.join("HTTPStorages"), bid) {
            push(AppDataKind::HttpStorages, p);
        }
        for p in prefixed_children(&lib.join("LaunchAgents"), bid) {
            push(AppDataKind::LaunchAgents, p);
        }
        for p in prefixed_children(&lib.join("Logs/DiagnosticReports"), bid) {
            push(AppDataKind::CrashReports, p);
        }
    }

    // Name-based locations. Electron apps in particular store data under their
    // `CFBundleName` ("Code" for Visual Studio Code) rather than the bundle identifier,
    // so every known name is tried. Only exact matches, never prefixes.
    for name in data_names(app) {
        push(
            AppDataKind::ApplicationSupport,
            lib.join("Application Support").join(&name),
        );
        push(AppDataKind::Caches, lib.join("Caches").join(&name));
        push(AppDataKind::Logs, lib.join("Logs").join(&name));
    }
    out
}

/// Folder names an application may use for its data: the bundle file name plus the names
/// declared in `Info.plist`. Short names are dropped to avoid matching unrelated folders.
fn data_names(app: &ApplicationInfo) -> Vec<String> {
    let bm = read_info_plist(Path::new(&app.path));
    let mut names = vec![app.name.trim().to_string()];
    names.extend(bm.name);
    names.retain(|n| n.len() >= 3 && !n.contains('/'));
    names.sort();
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_app(home: &Path, name: &str, bundle_id: &str) -> ApplicationInfo {
        ApplicationInfo {
            id: "x".into(),
            name: name.into(),
            path: home
                .join("Applications")
                .join(format!("{name}.app"))
                .to_string_lossy()
                .into(),
            version: None,
            bundle_id: Some(bundle_id.into()),
            publisher: None,
            size_bytes: None,
            modified_at: None,
            source: "applications".into(),
            is_system: false,
            uninstall_command: None,
        }
    }

    #[test]
    fn related_paths_match_bundle_id_and_bundle_name() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().to_path_buf();
        let lib = home.join("Library");
        // The bundle itself, with an Info.plist whose CFBundleName differs from the file name.
        let bundle = home.join("Applications/Visual Studio Code.app");
        std::fs::create_dir_all(bundle.join("Contents")).unwrap();
        std::fs::write(
            bundle.join("Contents/Info.plist"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Code</string>
<key>CFBundleIdentifier</key><string>com.microsoft.VSCode</string>
</dict></plist>"#,
        )
        .unwrap();
        // Data in both styles, plus decoys that must NOT be picked up.
        std::fs::create_dir_all(lib.join("Application Support/Code/User")).unwrap();
        std::fs::create_dir_all(lib.join("Caches/com.microsoft.VSCode")).unwrap();
        std::fs::create_dir_all(lib.join("Preferences")).unwrap();
        std::fs::write(lib.join("Preferences/com.microsoft.VSCode.plist"), b"x").unwrap();
        std::fs::write(
            lib.join("Preferences/com.microsoft.VSCodeOther.plist"),
            b"x",
        )
        .unwrap();
        std::fs::create_dir_all(lib.join("Application Support/CodeOther")).unwrap();

        let known = KnownPaths {
            home: home.clone(),
            ..Default::default()
        };
        let app = fake_app(&home, "Visual Studio Code", "com.microsoft.VSCode");
        let found = related_paths(&app, &known);
        let paths: Vec<&Path> = found.iter().map(|r| r.path.as_path()).collect();

        assert!(paths.contains(&bundle.as_path()));
        assert!(paths.contains(&lib.join("Application Support/Code").as_path()));
        assert!(paths.contains(&lib.join("Caches/com.microsoft.VSCode").as_path()));
        assert!(paths.contains(&lib.join("Preferences/com.microsoft.VSCode.plist").as_path()));
        // Decoys stay untouched.
        assert!(!paths.contains(
            &lib.join("Preferences/com.microsoft.VSCodeOther.plist")
                .as_path()
        ));
        assert!(!paths.contains(&lib.join("Application Support/CodeOther").as_path()));
    }

    #[test]
    fn short_names_are_not_used_for_folder_matching() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().to_path_buf();
        std::fs::create_dir_all(home.join("Library/Application Support/Go")).unwrap();
        let known = KnownPaths {
            home: home.clone(),
            ..Default::default()
        };
        let app = fake_app(&home, "Go", "com.example.go");
        let found = related_paths(&app, &known);
        assert!(found
            .iter()
            .all(|r| !r.path.ends_with("Application Support/Go")));
    }
}
