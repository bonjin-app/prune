//! Walks the user's project directories looking for build artifacts that can be regenerated:
//! `node_modules`, Rust `target/`, `build/`, `dist/`, Python caches, virtualenvs, …
//!
//! A directory is only reported when a *marker* file proves it belongs to a project
//! (e.g. `node_modules` next to `package.json`), so unrelated folders with the same name are
//! left alone. Found artifacts are never descended into.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use walkdir::WalkDir;

use chrono::{DateTime, Utc};

use crate::models::{Category, RiskLevel, TargetGroup};
use crate::platform::KnownPaths;
use crate::providers::{CleanupProvider, ScanContext, ScanOutput};
use crate::safety::git_tracked::{self, GitEvidence};

/// Maximum depth below a project root to look for artifacts.
const MAX_DEPTH: usize = 8;

/// Directories never descended into.
const SKIP_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "Library",
    "Applications",
    ".Trash",
    "node_modules",
    "Pods",
    ".venv",
    "venv",
    "vendor",
    ".idea",
    ".vscode",
    "$RECYCLE.BIN",
    "System Volume Information",
];

struct Rule {
    /// Directory name to match.
    dir: &'static str,
    /// One of these must exist next to the directory (empty = always).
    markers: &'static [&'static str],
    kind: &'static str,
    risk: RiskLevel,
}

const RULES: &[Rule] = &[
    Rule {
        dir: "node_modules",
        markers: &["package.json"],
        kind: "node_modules",
        risk: RiskLevel::Low,
    },
    Rule {
        dir: "target",
        markers: &["Cargo.toml"],
        kind: "Rust target",
        risk: RiskLevel::Low,
    },
    Rule {
        dir: "build",
        markers: &[
            "build.gradle",
            "build.gradle.kts",
            "settings.gradle",
            "settings.gradle.kts",
            "CMakeLists.txt",
            "pubspec.yaml",
            "package.json",
        ],
        kind: "build",
        risk: RiskLevel::Low,
    },
    Rule {
        dir: "dist",
        markers: &["package.json", "pyproject.toml", "setup.py"],
        kind: "dist",
        risk: RiskLevel::Low,
    },
    Rule {
        dir: "out",
        markers: &["package.json", "tsconfig.json"],
        kind: "out",
        risk: RiskLevel::Low,
    },
    Rule {
        dir: ".next",
        markers: &["package.json"],
        kind: "Next.js cache",
        risk: RiskLevel::Low,
    },
    Rule {
        dir: ".nuxt",
        markers: &["package.json"],
        kind: "Nuxt cache",
        risk: RiskLevel::Low,
    },
    Rule {
        dir: ".output",
        markers: &["package.json"],
        kind: "Nitro output",
        risk: RiskLevel::Low,
    },
    Rule {
        dir: ".turbo",
        markers: &["package.json"],
        kind: "Turborepo cache",
        risk: RiskLevel::Safe,
    },
    Rule {
        dir: ".parcel-cache",
        markers: &["package.json"],
        kind: "Parcel cache",
        risk: RiskLevel::Safe,
    },
    Rule {
        dir: ".expo",
        markers: &["package.json", "app.json"],
        kind: "Expo cache",
        risk: RiskLevel::Safe,
    },
    Rule {
        dir: "coverage",
        markers: &["package.json", "pyproject.toml"],
        kind: "coverage",
        risk: RiskLevel::Safe,
    },
    Rule {
        dir: ".gradle",
        markers: &[
            "build.gradle",
            "build.gradle.kts",
            "settings.gradle",
            "settings.gradle.kts",
        ],
        kind: "Gradle project cache",
        risk: RiskLevel::Low,
    },
    Rule {
        dir: ".build",
        markers: &["Package.swift"],
        kind: "SwiftPM build",
        risk: RiskLevel::Low,
    },
    Rule {
        dir: ".dart_tool",
        markers: &["pubspec.yaml"],
        kind: "Dart tool cache",
        risk: RiskLevel::Low,
    },
    Rule {
        dir: "Pods",
        markers: &["Podfile"],
        kind: "CocoaPods",
        risk: RiskLevel::Low,
    },
    Rule {
        dir: "__pycache__",
        markers: &[],
        kind: "Python bytecode",
        risk: RiskLevel::Safe,
    },
    Rule {
        dir: ".pytest_cache",
        markers: &[],
        kind: "pytest cache",
        risk: RiskLevel::Safe,
    },
    Rule {
        dir: ".mypy_cache",
        markers: &[],
        kind: "mypy cache",
        risk: RiskLevel::Safe,
    },
    Rule {
        dir: ".ruff_cache",
        markers: &[],
        kind: "ruff cache",
        risk: RiskLevel::Safe,
    },
    Rule {
        dir: ".tox",
        markers: &["tox.ini", "pyproject.toml"],
        kind: "tox environments",
        risk: RiskLevel::Low,
    },
    Rule {
        dir: ".venv",
        markers: &["pyvenv.cfg:inside"],
        kind: "Python virtualenv",
        risk: RiskLevel::Medium,
    },
    Rule {
        dir: "venv",
        markers: &["pyvenv.cfg:inside"],
        kind: "Python virtualenv",
        risk: RiskLevel::Medium,
    },
    Rule {
        dir: ".terraform",
        markers: &[],
        kind: "Terraform providers",
        risk: RiskLevel::Low,
    },
];

const UNITY_LIBRARY_RULE: Rule = Rule {
    dir: "Library",
    markers: &[], // Unity requires both directory markers, checked separately below.
    kind: "Unity Library cache",
    risk: RiskLevel::Low,
};

fn is_unity_project(dir: &Path) -> bool {
    ["Assets", "ProjectSettings"].iter().all(|name| {
        std::fs::symlink_metadata(dir.join(name))
            .map(|meta| meta.is_dir())
            .unwrap_or(false)
    })
}

/// Unity source/configuration stays protected even when a scan starts inside it.
/// Only a source-named ancestor with both real sibling markers establishes this boundary.
fn is_unity_source_path(path: &Path) -> bool {
    path.ancestors().any(|dir| {
        dir.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| ["Assets", "Packages", "ProjectSettings"].contains(&name))
            && dir.parent().is_some_and(is_unity_project)
    })
}

fn matching_rule(dir: &Path) -> Option<&'static Rule> {
    // Keep the candidate boundary independent of where discovery began or which rule matches.
    if is_unity_source_path(dir) {
        return None;
    }
    let name = dir.file_name()?.to_str()?;
    let parent = dir.parent()?;
    if name == UNITY_LIBRARY_RULE.dir {
        return is_unity_project(parent).then_some(&UNITY_LIBRARY_RULE);
    }
    RULES.iter().find(|r| {
        r.dir == name
            && (r.markers.is_empty()
                || r.markers.iter().any(|m| match m.strip_suffix(":inside") {
                    Some(inner) => dir.join(inner).exists(),
                    None => parent.join(m).exists(),
                }))
    })
}

/// `project/node_modules` style label relative to the scanned root.
fn label_for(root: &Path, dir: &Path) -> String {
    let rel = dir.strip_prefix(root).unwrap_or(dir);
    let comps: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    if comps.len() <= 3 {
        comps.join("/")
    } else {
        format!("…/{}", comps[comps.len() - 3..].join("/"))
    }
}

pub struct ProjectArtifacts;

impl ProjectArtifacts {
    pub fn roots(known: &KnownPaths) -> Vec<PathBuf> {
        known.project_roots.clone()
    }
}

impl CleanupProvider for ProjectArtifacts {
    fn id(&self) -> &str {
        "project_artifacts"
    }
    fn name(&self) -> &str {
        "Project Artifacts"
    }
    fn category(&self) -> Category {
        Category::DeveloperFiles
    }
    fn description(&self) -> &str {
        "node_modules, target/, build/, dist/, Python caches and similar regenerable folders inside your project directories."
    }
    fn default_risk(&self) -> RiskLevel {
        RiskLevel::Low
    }
    fn priority(&self) -> u8 {
        80
    }
    fn is_available(&self, known: &KnownPaths) -> bool {
        !known.project_roots.is_empty()
    }
    fn guarded_by_git(&self) -> bool {
        true
    }

    fn scan(&self, ctx: &ScanContext<'_>) -> ScanOutput {
        let (candidates, mut out) = self.discover(ctx);
        let git = ctx.platform.git();

        // Hundreds of artifacts, most of them small: measuring them one after another leaves
        // most cores idle even though each measurement is itself parallel. Measuring across
        // artifacts as well keeps the machine busy for the whole scan.
        let measured: Vec<(crate::providers::Measured, GitEvidence)> = candidates
            .par_iter()
            .map(|c| {
                let mut measured = crate::providers::measure(
                    ctx,
                    self.id(),
                    &c.path,
                    c.label.clone(),
                    c.risk,
                    Some(c.kind.to_string()),
                    false,
                );
                let mut evidence = GitEvidence::NotInRepository;
                if let Some(target) = measured.target.as_mut() {
                    target.group = c.group.clone();
                    // Already kept, by the user or the safety policy: Git would add nothing.
                    if target.risk != RiskLevel::Protected {
                        evidence = git_tracked::inspect(git.as_deref(), &c.path, ctx.cancel);
                    }
                    if let Some(reason) = kept_because(&evidence) {
                        target.risk = RiskLevel::Protected;
                        // In parentheses, because the kinds of a project are listed with commas.
                        target.description = Some(format!("{} ({reason})", c.kind));
                    } else if target.risk < RiskLevel::High && SHARED_NAMES.contains(&c.kind) {
                        if let Some(found) = kept_files(&c.path, ctx.cancel) {
                            target.risk = RiskLevel::High;
                            target.description = Some(format!("{} (holds {found})", c.kind));
                        }
                    }
                }
                (measured, evidence)
            })
            .collect();
        let mut without_git = None;
        for (m, evidence) in measured {
            out.issues.extend(m.issues);
            if let Some(target) = m.target {
                match evidence {
                    GitEvidence::Unknown(_) if git.is_none() => {
                        without_git.get_or_insert_with(|| target.path_buf());
                    }
                    GitEvidence::Unknown(reason) => out.issue(
                        &target.path_buf(),
                        format!("Kept: could not check it for files committed to Git. {reason}"),
                    ),
                    _ => {}
                }
                out.targets.push(target);
            }
        }
        // Once, not once per folder: every folder inside a repository is affected alike.
        if let Some(first) = without_git {
            out.issue(
                &first,
                "Git was not found, so build folders inside Git repositories could not be checked \
                 for committed files and are kept. Install Git to have them offered.",
            );
        }
        // Parallel measurement finishes in arbitrary order; sort so the same machine always
        // produces the same list.
        out.targets.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }
}

/// Folder names build tools use that people also use to keep things: release notes and
/// evidence, database dumps before a deploy, signing material. A build writes none of those, so
/// finding one means the folder is not only build output.
const SHARED_NAMES: &[&str] = &["dist", "build", "out"];

/// How many entries to look at before deciding a folder holds only build output. Enough for any
/// real `dist/`; a folder bigger than that is checked as far as this goes.
const KEPT_FILES_LOOK: usize = 50_000;

/// What `dir` holds that a build does not make, described for the user, or `None`.
///
/// Two signs, both of things people put there by hand or by script:
/// - folders named by date (`2026-10-14`, `run-20261021`), which build tools never create;
/// - files that are data or secrets rather than output: database dumps and backups, signing
///   keys, and `.env` files (not `.env.example`). Names with `.private.` count
///   too, as a person marking a file private is plainly keeping it.
pub(crate) fn kept_files(dir: &Path, cancel: &std::sync::atomic::AtomicBool) -> Option<String> {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.file_type().is_ok_and(|t| t.is_dir()) && is_dated(&name) {
                return Some(format!("folders named by date, such as {name}"));
            }
        }
    }
    let mut walker = WalkDir::new(dir)
        .follow_links(false)
        .max_depth(8)
        .into_iter();
    let mut looked = 0;
    while let Some(entry) = walker.next() {
        let Ok(entry) = entry else { continue };
        looked += 1;
        if looked > KEPT_FILES_LOOK || cancel.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        let name = entry.file_name().to_string_lossy();
        if entry.file_type().is_dir() {
            if name == "node_modules" {
                walker.skip_current_dir();
            }
            continue;
        }
        if is_kept_file(&name) {
            let shown = entry
                .path()
                .strip_prefix(dir)
                .unwrap_or(entry.path())
                .display()
                .to_string();
            return Some(format!("files a build does not make, such as {shown}"));
        }
    }
    None
}

/// A date in a folder name: `2026-10-14`, or eight digits from `20` such as `20261021`,
/// optionally followed by the time as six more (`20260417215624`).
fn is_dated(name: &str) -> bool {
    let b = name.as_bytes();
    let digits = |s: &[u8]| s.iter().all(u8::is_ascii_digit);
    (0..b.len()).any(|i| {
        let rest = &b[i..];
        let plausible = |y: &[u8], m: &[u8], d: &[u8]| {
            y.starts_with(b"20")
                && digits(y)
                && digits(m)
                && digits(d)
                && (b"01".as_slice()..=b"12".as_slice()).contains(&m)
                && (b"01".as_slice()..=b"31".as_slice()).contains(&d)
        };
        let boundary = |at: usize| at >= b.len() || !b[at].is_ascii_digit();
        let starts = i == 0 || !b[i - 1].is_ascii_digit();
        starts
            && ((rest.len() >= 10
                && rest[4] == b'-'
                && rest[7] == b'-'
                && plausible(&rest[..4], &rest[5..7], &rest[8..10])
                && boundary(i + 10))
                || (rest.len() >= 8
                    && plausible(&rest[..4], &rest[4..6], &rest[6..8])
                    && (boundary(i + 8)
                        || (rest.len() >= 14 && digits(&rest[8..14]) && boundary(i + 14)))))
    })
}

fn is_kept_file(name: &str) -> bool {
    // Builds copy database files too (migrations into `resources/`, a local cache's
    // `metadata.sqlite`), so a database file counts only when its name says it is a copy kept
    // on purpose. Dumps and backups count whatever they are called.
    const DATABASE: &[&str] = &["sql", "sqlite", "sqlite3", "db"];
    const KEPT_WORDS: &[&str] = &["backup", "dump", "before", "snapshot", "export", "prod"];
    const ALWAYS: &[&str] = &["dump", "backup"];
    // Signing material. Not `.pem`: bundled certificate stores are public and builds ship them.
    const SECRETS: &[&str] = &["key", "p12", "pfx", "keystore", "jks", "mobileprovision"];
    let lower = name.to_ascii_lowercase();
    if lower == ".env" || (lower.starts_with(".env.") && !is_env_template(&lower)) {
        return true;
    }
    let Some((stem, ext)) = lower.rsplit_once('.') else {
        return false;
    };
    // Compilers name some of their own output `.private.` (Swift's private module interface).
    const COMPILER_OUTPUT: &[&str] = &["swiftinterface", "modulemap", "swiftmodule", "swiftdoc"];
    if lower.contains(".private.") && !COMPILER_OUTPUT.contains(&ext) {
        return true;
    }
    if stem.is_empty() {
        return false;
    }
    ALWAYS.contains(&ext)
        || SECRETS.contains(&ext)
        || (DATABASE.contains(&ext) && KEPT_WORDS.iter().any(|w| stem.contains(w)))
}

fn is_env_template(lower: &str) -> bool {
    [".example", ".sample", ".template", ".dist", ".defaults"]
        .iter()
        .any(|s| lower.ends_with(s))
}

/// Why a build folder is kept instead of offered, or `None` when Git has no objection.
fn kept_because(evidence: &GitEvidence) -> Option<&'static str> {
    match evidence {
        GitEvidence::NotInRepository | GitEvidence::Untracked => None,
        GitEvidence::Tracked => Some("committed to Git"),
        GitEvidence::Repository => Some("its own Git repository"),
        GitEvidence::Unknown(_) => Some("not checked with Git"),
    }
}

/// One artifact directory found by the walk, before it has been measured.
struct Candidate {
    path: PathBuf,
    label: String,
    kind: &'static str,
    risk: RiskLevel,
    group: Option<TargetGroup>,
}

/// Finds the project an artifact belongs to, and when that project was last worked on.
///
/// The answer for `queryx/apps/desktop/src-tauri/target` is `queryx`, not `src-tauri`: the
/// project is the nearest ancestor holding a `.git` directory. Without walking up, a monorepo
/// or a Tauri app scatters its artifacts across a dozen unrelated-looking rows.
struct ProjectLookup {
    root: PathBuf,
    /// One answer per directory, because sibling artifacts share a project.
    cache: HashMap<PathBuf, Option<TargetGroup>>,
}

impl ProjectLookup {
    fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            cache: HashMap::new(),
        }
    }

    fn group_for(&mut self, artifact: &Path) -> Option<TargetGroup> {
        let start = artifact.parent()?.to_path_buf();
        if let Some(cached) = self.cache.get(&start) {
            return cached.clone();
        }
        let group = self.compute(&start);
        self.cache.insert(start, group.clone());
        group
    }

    fn compute(&self, start: &Path) -> Option<TargetGroup> {
        let mut current = Some(start);
        while let Some(dir) = current {
            if dir.join(".git").exists() {
                return Some(Self::describe(dir));
            }
            if dir == self.root {
                break;
            }
            current = dir.parent();
        }
        // No repository above it: the directory the artifact sits in is as good as it gets.
        Some(Self::describe(start))
    }

    fn describe(dir: &Path) -> TargetGroup {
        TargetGroup {
            key: dir.to_string_lossy().into_owned(),
            label: dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| dir.to_string_lossy().into_owned()),
            last_active_at: Self::last_active(dir),
        }
    }

    /// When someone last did something in this repository.
    ///
    /// `.git/index` moves on every stage, commit, checkout or status that refreshes it, which
    /// tracks real work far better than the mtime of the directory itself. `HEAD` is the
    /// fallback, and a project with no repository has no answer at all — which is reported as
    /// unknown rather than as "never", because guessing here would age a project wrongly.
    fn last_active(dir: &Path) -> Option<DateTime<Utc>> {
        ["index", "HEAD"]
            .iter()
            .filter_map(|name| std::fs::metadata(dir.join(".git").join(name)).ok())
            .filter_map(|meta| meta.modified().ok())
            .map(DateTime::<Utc>::from)
            .max()
    }
}

impl ProjectArtifacts {
    /// Walks the project roots looking for artifact directories. Cheap compared with measuring
    /// them, and never descends into one it has already reported.
    fn discover(&self, ctx: &ScanContext<'_>) -> (Vec<Candidate>, ScanOutput) {
        let mut out = ScanOutput::default();
        let mut candidates = Vec::new();
        let mut visited: u64 = 0;
        for root in Self::roots(ctx.known) {
            // WalkDir follows root symlinks even with follow_links(false). Check both the
            // supplied root and its resolved ancestry before walking any source directory.
            if is_unity_source_path(&root)
                || std::fs::canonicalize(&root)
                    .is_ok_and(|resolved| is_unity_source_path(&resolved))
            {
                continue;
            }
            let mut projects = ProjectLookup::new(&root);
            let mut walker = WalkDir::new(&root)
                .follow_links(false)
                .max_depth(MAX_DEPTH)
                .sort_by_file_name()
                .into_iter();
            while let Some(entry) = walker.next() {
                if ctx.is_cancelled() {
                    return (candidates, out);
                }
                let entry = match entry {
                    Ok(e) => e,
                    Err(e) => {
                        if let Some(p) = e.path() {
                            out.issue(p, e.to_string());
                        }
                        continue;
                    }
                };
                if !entry.file_type().is_dir() || entry.depth() == 0 {
                    continue;
                }
                visited += 1;
                if visited % 64 == 0 {
                    (ctx.progress)(visited, 0, entry.path());
                }
                let path = entry.path();
                let name = entry.file_name().to_string_lossy();
                // Source assets and project configuration must remain intact, including
                // nested directories whose names happen to match another artifact rule.
                if is_unity_source_path(path) {
                    walker.skip_current_dir();
                    continue;
                }
                if let Some(rule) = matching_rule(path) {
                    candidates.push(Candidate {
                        label: label_for(&root, path),
                        group: projects.group_for(path),
                        path: path.to_path_buf(),
                        kind: rule.kind,
                        risk: rule.risk,
                    });
                    walker.skip_current_dir();
                    continue;
                }
                if SKIP_DIRS.contains(&name.as_ref()) || (name.starts_with('.') && name != ".") {
                    walker.skip_current_dir();
                }
            }
        }
        (candidates, out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_in_folder_names() {
        for yes in [
            "2026-10-14",
            "run-20261021",
            "admin-ops-2026-10-15",
            "x_20260417215624",
        ] {
            assert!(is_dated(yes), "{yes}");
        }
        for no in [
            "assets",
            "v2",
            "2026",
            "chunk-1234567890",
            "20261399",
            "build-123",
        ] {
            assert!(!is_dated(no), "{no}");
        }
    }

    #[test]
    fn data_and_secrets_are_not_build_output() {
        for yes in [
            "d1-before.private.sql",
            "backup.sqlite",
            "prod.db",
            "nightly-dump.sql",
            "users.dump",
            "release.keystore",
            ".env",
            ".env.production",
            "deploy.private.log",
            "cert.p12",
        ] {
            assert!(is_kept_file(yes), "{yes}");
        }
        for no in [
            "index.js",
            "app.js.map",
            ".env.example",
            ".env.sample",
            "main.css",
            "db",
            "bundle.aab",
            // What builds copy into their output, seen in real build folders:
            "V20__add_column.sql",
            "data.sql",
            "Thumbs.db",
            "metadata.sqlite",
            "cacert.pem",
            "module.modulemap.bak",
            "arm64-apple-ios-simulator.private.swiftinterface",
            "module.private.modulemap",
        ] {
            assert!(!is_kept_file(no), "{no}");
        }
    }

    #[test]
    fn a_dist_folder_used_to_keep_things_is_found() {
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let built = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(built.path().join("assets")).unwrap();
        std::fs::write(built.path().join("assets/index-3f2a.js"), "x").unwrap();
        std::fs::write(built.path().join(".env.example"), "x").unwrap();
        assert_eq!(kept_files(built.path(), &cancel), None);

        let dated = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dated.path().join("release-evidence-20261031")).unwrap();
        assert!(kept_files(dated.path(), &cancel)
            .unwrap()
            .contains("by date"));

        let dump = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dump.path().join("deploy")).unwrap();
        std::fs::write(dump.path().join("deploy/d1-before.private.sql"), "x").unwrap();
        let found = kept_files(dump.path(), &cancel).unwrap();
        assert!(found.contains("d1-before.private.sql"), "{found}");
    }

    #[test]
    fn rule_requires_marker() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("app");
        std::fs::create_dir_all(project.join("node_modules")).unwrap();
        assert!(matching_rule(&project.join("node_modules")).is_none());
        std::fs::write(project.join("package.json"), "{}").unwrap();
        assert_eq!(
            matching_rule(&project.join("node_modules")).unwrap().kind,
            "node_modules"
        );
    }

    #[test]
    fn unity_library_requires_both_sibling_directory_markers() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path();
        let library = project.join("Library");
        std::fs::create_dir(&library).unwrap();
        assert!(matching_rule(&library).is_none());
        std::fs::create_dir(project.join("Assets")).unwrap();
        assert!(matching_rule(&library).is_none());
        std::fs::write(project.join("ProjectSettings"), "not a directory").unwrap();
        assert!(matching_rule(&library).is_none());
        std::fs::remove_file(project.join("ProjectSettings")).unwrap();
        std::fs::create_dir(project.join("ProjectSettings")).unwrap();
        let rule = matching_rule(&library).unwrap();
        assert_eq!(rule.kind, "Unity Library cache");
        assert_eq!(rule.risk, RiskLevel::Low);
        std::fs::remove_dir(project.join("Assets")).unwrap();
        assert!(matching_rule(&library).is_none());
        std::fs::write(project.join("Assets"), "not a directory").unwrap();
        assert!(matching_rule(&library).is_none());
        let nested = library.join("Library");
        std::fs::create_dir(&nested).unwrap();
        assert!(matching_rule(&nested).is_none());
    }

    #[test]
    fn unity_source_candidates_are_refused_directly_at_any_depth() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path();
        std::fs::create_dir(project.join("Assets")).unwrap();
        std::fs::create_dir(project.join("ProjectSettings")).unwrap();
        for source in ["Assets", "Packages", "ProjectSettings"] {
            let parent = project.join(source).join("tools/nested");
            std::fs::create_dir_all(&parent).unwrap();
            std::fs::write(parent.join("package.json"), "{}").unwrap();
            for artifact in [
                "dist",
                "node_modules",
                "build",
                "__pycache__",
                ".pytest_cache",
            ] {
                let candidate = parent.join(artifact);
                std::fs::create_dir(&candidate).unwrap();
                assert!(
                    matching_rule(&candidate).is_none(),
                    "{}",
                    candidate.display()
                );
            }
        }
        let embedded = project.join("Assets/EmbeddedProject");
        for directory in ["Assets", "ProjectSettings", "Library"] {
            std::fs::create_dir_all(embedded.join(directory)).unwrap();
        }
        assert!(matching_rule(&embedded.join("Library")).is_none());
        let library = project.join("Library");
        std::fs::create_dir(&library).unwrap();
        assert_eq!(matching_rule(&library).unwrap().kind, "Unity Library cache");
    }

    #[cfg(unix)]
    #[test]
    fn unity_directory_markers_must_not_be_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        std::fs::create_dir_all(project.join("Library")).unwrap();
        std::fs::create_dir(project.join("ProjectSettings")).unwrap();
        let assets = dir.path().join("external-assets");
        std::fs::create_dir(&assets).unwrap();
        std::os::unix::fs::symlink(assets, project.join("Assets")).unwrap();
        assert!(matching_rule(&project.join("Library")).is_none());
    }

    #[test]
    fn an_artifact_belongs_to_the_repository_above_it_not_its_own_folder() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        // A monorepo: the repository is at the top, the artifact is three levels down.
        let repo = root.join("queryx");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::write(repo.join(".git/index"), b"x").unwrap();
        let artifact = repo.join("apps/desktop/src-tauri/target");
        std::fs::create_dir_all(&artifact).unwrap();

        let mut lookup = ProjectLookup::new(&root);
        let group = lookup.group_for(&artifact).unwrap();

        assert_eq!(group.label, "queryx");
        assert_eq!(group.key, repo.to_string_lossy());
        assert!(
            group.last_active_at.is_some(),
            "the repository's own activity should be readable"
        );
    }

    #[test]
    fn artifacts_in_one_project_share_a_group() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let repo = root.join("app");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::create_dir_all(repo.join("node_modules")).unwrap();
        std::fs::create_dir_all(repo.join("packages/ui/dist")).unwrap();

        let mut lookup = ProjectLookup::new(&root);
        let a = lookup.group_for(&repo.join("node_modules")).unwrap();
        let b = lookup.group_for(&repo.join("packages/ui/dist")).unwrap();

        assert_eq!(a.key, b.key, "both belong to the same project");
        assert_eq!(a.label, "app");
    }

    #[test]
    fn a_project_without_a_repository_still_gets_a_group_but_no_date() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let project = root.join("loose/thing");
        std::fs::create_dir_all(project.join("node_modules")).unwrap();

        let mut lookup = ProjectLookup::new(&root);
        let group = lookup.group_for(&project.join("node_modules")).unwrap();

        assert_eq!(group.label, "thing");
        assert_eq!(
            group.last_active_at, None,
            "an unknown date must not be reported as a very old one"
        );
    }

    #[test]
    fn the_search_stops_at_the_scan_root() {
        let dir = tempfile::tempdir().unwrap();
        // A repository *above* the directory being scanned must not swallow everything in it.
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        let root = dir.path().join("projects");
        let project = root.join("app");
        std::fs::create_dir_all(project.join("node_modules")).unwrap();

        let mut lookup = ProjectLookup::new(&root);
        let group = lookup.group_for(&project.join("node_modules")).unwrap();

        assert_eq!(group.label, "app", "grouping must not escape the scan root");
    }

    #[test]
    fn venv_requires_pyvenv_cfg_inside() {
        let dir = tempfile::tempdir().unwrap();
        let venv = dir.path().join(".venv");
        std::fs::create_dir_all(&venv).unwrap();
        assert!(matching_rule(&venv).is_none());
        std::fs::write(venv.join("pyvenv.cfg"), "").unwrap();
        assert_eq!(matching_rule(&venv).unwrap().risk, RiskLevel::Medium);
    }
}
