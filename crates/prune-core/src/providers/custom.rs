//! Providers defined in a file instead of in Rust.
//!
//! The built-in list covers the tools most developers have, but nobody's machine is the list.
//! Adding a location should not mean forking the project and rebuilding it, so `providers.json`
//! in the application data directory is read at start-up and its entries join the built-in ones.
//!
//! What a file cannot do is bypass anything. A custom provider only chooses *where to look*;
//! every path it produces goes through the same [`SafetyPolicy`](crate::safety::SafetyPolicy)
//! as a built-in one, so a file cannot point Prune at the system, and every target still needs
//! a preview and a confirmation before anything happens.
//!
//! ```json
//! {
//!   "providers": [
//!     {
//!       "id": "zig_cache",
//!       "name": "Zig cache",
//!       "description": "Compiler cache. Rebuilt on the next build.",
//!       "category": "developer_files",
//!       "risk": "safe",
//!       "mode": "whole",
//!       "paths": ["{cache}/zig", "~/.cache/zig"]
//!     }
//!   ]
//! }
//! ```

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{CleanupProvider, RootMode, ScanContext, ScanOutput};
use crate::models::{Category, RiskLevel};
use crate::platform::KnownPaths;

/// The file Prune reads custom providers from, inside the application data directory.
pub const FILE_NAME: &str = "providers.json";

/// One provider as written in the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CustomProviderSpec {
    /// `snake_case`, unique, and not one of the built-in ids.
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub category: Category,
    pub risk: RiskLevel,
    /// `whole` treats each path as one item; `children` treats each entry inside it as one.
    #[serde(default)]
    pub mode: CustomMode,
    /// Where to look. Supports `~` and `{home}`, `{cache}`, `{appSupport}`, `{localAppData}`,
    /// `{temp}`, `{downloads}`, `{logs}`.
    pub paths: Vec<String>,
    /// Only offer entries untouched for at least this long.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_age_days: Option<u32>,
    /// Names to skip in `children` mode.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustomMode {
    #[default]
    Whole,
    Children,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CustomFile {
    #[serde(default)]
    providers: Vec<CustomProviderSpec>,
}

/// Something wrong with the file, reported rather than silently ignored.
///
/// A provider that quietly failed to load would look exactly like a provider that found
/// nothing, and the user would conclude their cache is already clean.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomIssue {
    /// The offending provider id, when it got far enough to have one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub message: String,
}

/// What was read from the file.
#[derive(Debug, Default)]
pub struct CustomProviders {
    pub providers: Vec<CustomProvider>,
    pub issues: Vec<CustomIssue>,
}

/// Reads `providers.json` from `dir`.
///
/// A missing file is the normal case and yields nothing at all. Anything else that goes wrong
/// becomes an issue the UI can show.
pub fn load(dir: &Path, reserved: &[&str]) -> CustomProviders {
    let path = dir.join(FILE_NAME);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return CustomProviders::default(),
        Err(e) => {
            return CustomProviders {
                providers: Vec::new(),
                issues: vec![CustomIssue {
                    id: None,
                    message: format!("{} could not be read: {e}", path.display()),
                }],
            }
        }
    };
    parse(&text, reserved)
}

/// Parses the file's contents. Separated from reading so it can be tested directly.
pub fn parse(text: &str, reserved: &[&str]) -> CustomProviders {
    let file: CustomFile = match serde_json::from_str(text) {
        Ok(file) => file,
        Err(e) => {
            return CustomProviders {
                providers: Vec::new(),
                issues: vec![CustomIssue {
                    id: None,
                    message: format!("{FILE_NAME} is not valid: {e}"),
                }],
            }
        }
    };

    let mut providers = Vec::new();
    let mut issues = Vec::new();
    let mut seen: Vec<String> = Vec::new();

    for spec in file.providers {
        if let Err(message) = validate(&spec, reserved, &seen) {
            issues.push(CustomIssue {
                id: Some(spec.id),
                message,
            });
            continue;
        }
        seen.push(spec.id.clone());
        providers.push(CustomProvider { spec });
    }
    CustomProviders { providers, issues }
}

fn validate(spec: &CustomProviderSpec, reserved: &[&str], seen: &[String]) -> Result<(), String> {
    if spec.id.trim().is_empty() {
        return Err("id must not be empty".into());
    }
    if !spec
        .id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err("id may only contain lowercase letters, digits and underscores".into());
    }
    if reserved.contains(&spec.id.as_str()) {
        return Err(format!(
            "id '{}' is already used by a built-in provider",
            spec.id
        ));
    }
    if seen.contains(&spec.id) {
        return Err(format!("id '{}' appears more than once", spec.id));
    }
    if spec.name.trim().is_empty() {
        return Err("name must not be empty".into());
    }
    if spec.paths.is_empty() {
        return Err("at least one path is needed".into());
    }
    // A protected risk level would produce items nothing can ever select.
    if spec.risk == RiskLevel::Protected {
        return Err("risk 'protected' would make every item unusable".into());
    }
    for path in &spec.paths {
        if let Some(token) = unknown_token(path) {
            return Err(format!("unknown location '{{{token}}}' in \"{path}\""));
        }
    }
    Ok(())
}

/// The first `{token}` in `path` that Prune does not recognise.
fn unknown_token(path: &str) -> Option<&str> {
    let mut rest = path;
    while let Some(start) = rest.find('{') {
        let after = &rest[start + 1..];
        let end = after.find('}')?;
        let token = &after[..end];
        if resolve_token(token, &KnownPaths::default()).is_none() && !is_known_token(token) {
            return Some(token);
        }
        rest = &after[end + 1..];
    }
    None
}

fn is_known_token(token: &str) -> bool {
    matches!(
        token,
        "home" | "cache" | "appSupport" | "localAppData" | "temp" | "downloads" | "logs"
    )
}

fn resolve_token(token: &str, known: &KnownPaths) -> Option<PathBuf> {
    match token {
        "home" => Some(known.home.clone()),
        "cache" => known.user_cache.clone(),
        "appSupport" => known.app_support.clone(),
        "localAppData" => known.local_app_data.clone(),
        "temp" => Some(known.temp.clone()),
        "downloads" => known.downloads.clone(),
        "logs" => known.user_logs.clone(),
        _ => None,
    }
}

/// Turns one written path into a real one.
///
/// `None` when it names a location this platform does not have — `{logs}` on Windows, for
/// instance — which is not an error: a file meant for both platforms says where to look on
/// each, and the half that does not apply is simply skipped.
pub fn expand(path: &str, known: &KnownPaths) -> Option<PathBuf> {
    let expanded = if let Some(rest) = path.strip_prefix("~/") {
        known.home.join(rest)
    } else if path == "~" {
        known.home.clone()
    } else if path.starts_with('{') {
        let end = path.find('}')?;
        let base = resolve_token(&path[1..end], known)?;
        let rest = path[end + 1..].trim_start_matches(['/', '\\']);
        if rest.is_empty() {
            base
        } else {
            base.join(rest)
        }
    } else {
        PathBuf::from(path)
    };
    expanded.is_absolute().then_some(expanded)
}

/// A provider built from a file entry.
#[derive(Debug, Clone)]
pub struct CustomProvider {
    spec: CustomProviderSpec,
}

impl CustomProvider {
    pub fn spec(&self) -> &CustomProviderSpec {
        &self.spec
    }

    fn roots(&self, known: &KnownPaths) -> Vec<PathBuf> {
        self.spec
            .paths
            .iter()
            .filter_map(|p| expand(p, known))
            .filter(|p| p.exists())
            .collect()
    }

    fn old_enough(&self, path: &Path) -> bool {
        let Some(days) = self.spec.min_age_days else {
            return true;
        };
        let Ok(modified) = std::fs::symlink_metadata(path).and_then(|m| m.modified()) else {
            return false;
        };
        std::time::SystemTime::now()
            .duration_since(modified)
            .unwrap_or_default()
            >= std::time::Duration::from_secs(u64::from(days) * 86_400)
    }
}

impl CleanupProvider for CustomProvider {
    fn id(&self) -> &str {
        &self.spec.id
    }

    fn name(&self) -> &str {
        &self.spec.name
    }

    fn category(&self) -> Category {
        self.spec.category
    }

    fn description(&self) -> &str {
        &self.spec.description
    }

    fn default_risk(&self) -> RiskLevel {
        self.spec.risk
    }

    /// Above the built-in generic providers, so a location someone named by hand is reported
    /// under their own name rather than as an anonymous cache folder.
    fn priority(&self) -> u8 {
        90
    }

    fn is_custom(&self) -> bool {
        true
    }

    fn is_available(&self, known: &KnownPaths) -> bool {
        !self.roots(known).is_empty()
    }

    fn scan(&self, ctx: &ScanContext<'_>) -> ScanOutput {
        let mut out = ScanOutput::default();
        let mode = match self.spec.mode {
            CustomMode::Whole => RootMode::Whole,
            CustomMode::Children => RootMode::Children,
        };
        for root in self.roots(ctx.known) {
            if ctx.is_cancelled() {
                break;
            }
            match mode {
                RootMode::Whole => {
                    if !self.old_enough(&root) {
                        continue;
                    }
                    let label = super::simple::file_name(&root);
                    out.push_measured(ctx, self.id(), &root, label, self.spec.risk, None, false);
                }
                RootMode::Children => {
                    let Ok(entries) = std::fs::read_dir(&root) else {
                        out.issue(&root, "could not be read");
                        continue;
                    };
                    let mut children: Vec<PathBuf> =
                        entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
                    children.sort();
                    for child in children {
                        if ctx.is_cancelled() {
                            break;
                        }
                        let name = super::simple::file_name(&child);
                        if name == ".DS_Store" || self.spec.exclude.contains(&name) {
                            continue;
                        }
                        if !self.old_enough(&child) {
                            continue;
                        }
                        out.push_measured(
                            ctx,
                            self.id(),
                            &child,
                            name,
                            self.spec.risk,
                            None,
                            false,
                        );
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUILT_IN: &[&str] = &["npm_cache", "user_cache"];

    fn spec_json(body: &str) -> String {
        format!(r#"{{"providers":[{body}]}}"#)
    }

    fn valid(id: &str) -> String {
        format!(
            r#"{{"id":"{id}","name":"Thing","category":"developer_files","risk":"safe","paths":["~/thing"]}}"#
        )
    }

    #[test]
    fn a_missing_file_is_the_normal_case() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = load(dir.path(), BUILT_IN);
        assert!(loaded.providers.is_empty());
        assert!(loaded.issues.is_empty(), "{:?}", loaded.issues);
    }

    #[test]
    fn reads_a_provider_from_the_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE_NAME), spec_json(&valid("zig_cache"))).unwrap();

        let loaded = load(dir.path(), BUILT_IN);

        assert!(loaded.issues.is_empty(), "{:?}", loaded.issues);
        assert_eq!(loaded.providers.len(), 1);
        let provider = &loaded.providers[0];
        assert_eq!(provider.id(), "zig_cache");
        assert_eq!(provider.default_risk(), RiskLevel::Safe);
        assert!(provider.is_custom());
    }

    #[test]
    fn a_broken_file_is_reported_not_ignored() {
        // A provider that silently failed to load looks exactly like one that found nothing,
        // and the user would conclude the cache is already clean.
        let loaded = parse("{ not json", BUILT_IN);
        assert!(loaded.providers.is_empty());
        assert_eq!(loaded.issues.len(), 1);
        assert!(loaded.issues[0].message.contains("not valid"));
    }

    #[test]
    fn one_bad_entry_does_not_lose_the_good_ones() {
        let text = format!(
            r#"{{"providers":[{},{},{}]}}"#,
            valid("first"),
            r#"{"id":"BAD ID","name":"x","category":"logs","risk":"safe","paths":["~/x"]}"#,
            valid("second")
        );
        let loaded = parse(&text, BUILT_IN);

        assert_eq!(loaded.providers.len(), 2);
        assert_eq!(loaded.issues.len(), 1);
        assert_eq!(loaded.issues[0].id.as_deref(), Some("BAD ID"));
    }

    #[test]
    fn refuses_an_id_that_would_shadow_a_built_in_one() {
        let loaded = parse(&spec_json(&valid("npm_cache")), BUILT_IN);
        assert!(loaded.providers.is_empty());
        assert!(loaded.issues[0].message.contains("built-in"));
    }

    #[test]
    fn refuses_a_repeated_id() {
        let text = format!(r#"{{"providers":[{},{}]}}"#, valid("dup"), valid("dup"));
        let loaded = parse(&text, BUILT_IN);
        assert_eq!(loaded.providers.len(), 1);
        assert!(loaded.issues[0].message.contains("more than once"));
    }

    #[test]
    fn refuses_entries_that_could_never_work() {
        let cases = [
            (
                r#"{"id":"no_paths","name":"x","category":"logs","risk":"safe","paths":[]}"#,
                "at least one path",
            ),
            (
                r#"{"id":"no_name","name":"","category":"logs","risk":"safe","paths":["~/x"]}"#,
                "name must not be empty",
            ),
            (
                r#"{"id":"locked","name":"x","category":"logs","risk":"protected","paths":["~/x"]}"#,
                "unusable",
            ),
            (
                r#"{"id":"odd_token","name":"x","category":"logs","risk":"safe","paths":["{nowhere}/x"]}"#,
                "unknown location",
            ),
        ];
        for (body, expected) in cases {
            let loaded = parse(&spec_json(body), BUILT_IN);
            assert!(loaded.providers.is_empty(), "{body}");
            assert!(
                loaded.issues[0].message.contains(expected),
                "{body} -> {}",
                loaded.issues[0].message
            );
        }
    }

    #[test]
    fn rejects_fields_it_does_not_understand_rather_than_ignoring_them() {
        // A typo in a field name would otherwise change nothing and say nothing.
        let body = r#"{"id":"typo","name":"x","category":"logs","risk":"safe","paths":["~/x"],"minAge":3}"#;
        let loaded = parse(&spec_json(body), BUILT_IN);
        assert!(loaded.providers.is_empty());
        assert!(loaded.issues[0].message.contains("not valid"));
    }

    #[test]
    fn expands_the_locations_it_understands() {
        // Built from a root this platform calls absolute rather than written out as a Unix
        // path: `/home/u` has a root on Windows but is not absolute there — it is relative to
        // the current drive — so `expand` would rightly refuse it and the test would be
        // measuring the fixture instead of the behaviour.
        let root = if cfg!(windows) {
            PathBuf::from("C:\\")
        } else {
            PathBuf::from("/")
        };
        let home = root.join("home").join("u");
        let cache = home.join("Library").join("Caches");
        let temp = root.join("tmp");
        let known = KnownPaths {
            home: home.clone(),
            user_cache: Some(cache.clone()),
            temp: temp.clone(),
            ..Default::default()
        };

        assert_eq!(expand("~/thing", &known).unwrap(), home.join("thing"));
        assert_eq!(expand("~", &known).unwrap(), home);
        assert_eq!(expand("{cache}/zig", &known).unwrap(), cache.join("zig"));
        assert_eq!(expand("{temp}", &known).unwrap(), temp);

        let absolute = root.join("absolute").join("path");
        assert_eq!(
            expand(&absolute.to_string_lossy(), &known).unwrap(),
            absolute
        );
        // A relative path is not a location.
        assert_eq!(expand("relative/path", &known), None);
        // A location this platform does not have is skipped, not an error.
        assert_eq!(expand("{downloads}/x", &known), None);
    }

    #[test]
    fn a_custom_provider_finds_what_it_was_pointed_at() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        std::fs::create_dir_all(home.join("thing")).unwrap();
        std::fs::write(home.join("thing/blob.bin"), vec![0u8; 2_048]).unwrap();

        let loaded = parse(&spec_json(&valid("thing_cache")), BUILT_IN);
        let provider = &loaded.providers[0];
        let known = KnownPaths {
            home: home.clone(),
            ..Default::default()
        };

        assert!(provider.is_available(&known));
        assert_eq!(provider.roots(&known), vec![home.join("thing")]);
    }

    #[test]
    fn a_path_that_does_not_exist_makes_the_provider_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = parse(&spec_json(&valid("thing_cache")), BUILT_IN);
        let known = KnownPaths {
            home: dir.path().to_path_buf(),
            ..Default::default()
        };
        assert!(!loaded.providers[0].is_available(&known));
    }
}
