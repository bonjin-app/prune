//! Walks the user's project directories looking for build artifacts that can be regenerated:
//! `node_modules`, Rust `target/`, `build/`, `dist/`, Python caches, virtualenvs, …
//!
//! A directory is only reported when a *marker* file proves it belongs to a project
//! (e.g. `node_modules` next to `package.json`), so unrelated folders with the same name are
//! left alone. Found artifacts are never descended into.

use std::path::{Path, PathBuf};

use rayon::prelude::*;
use walkdir::WalkDir;

use crate::models::{Category, RiskLevel};
use crate::platform::KnownPaths;
use crate::providers::{CleanupProvider, ScanContext, ScanOutput};

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

fn matching_rule(dir: &Path) -> Option<&'static Rule> {
    let name = dir.file_name()?.to_str()?;
    let parent = dir.parent()?;
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
    fn id(&self) -> &'static str {
        "project_artifacts"
    }
    fn name(&self) -> &'static str {
        "Project Artifacts"
    }
    fn category(&self) -> Category {
        Category::DeveloperFiles
    }
    fn description(&self) -> &'static str {
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

    fn scan(&self, ctx: &ScanContext<'_>) -> ScanOutput {
        let (candidates, mut out) = self.discover(ctx);

        // Hundreds of artifacts, most of them small: measuring them one after another leaves
        // most cores idle even though each measurement is itself parallel. Measuring across
        // artifacts as well keeps the machine busy for the whole scan.
        let measured: Vec<crate::providers::Measured> = candidates
            .par_iter()
            .map(|c| {
                crate::providers::measure(
                    ctx,
                    self.id(),
                    &c.path,
                    c.label.clone(),
                    c.risk,
                    Some(c.kind.to_string()),
                    false,
                )
            })
            .collect();
        for m in measured {
            out.issues.extend(m.issues);
            if let Some(target) = m.target {
                out.targets.push(target);
            }
        }
        // Parallel measurement finishes in arbitrary order; sort so the same machine always
        // produces the same list.
        out.targets.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }
}

/// One artifact directory found by the walk, before it has been measured.
struct Candidate {
    path: PathBuf,
    label: String,
    kind: &'static str,
    risk: RiskLevel,
}

impl ProjectArtifacts {
    /// Walks the project roots looking for artifact directories. Cheap compared with measuring
    /// them, and never descends into one it has already reported.
    fn discover(&self, ctx: &ScanContext<'_>) -> (Vec<Candidate>, ScanOutput) {
        let mut out = ScanOutput::default();
        let mut candidates = Vec::new();
        let mut visited: u64 = 0;
        for root in Self::roots(ctx.known) {
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
                if let Some(rule) = matching_rule(path) {
                    candidates.push(Candidate {
                        label: label_for(&root, path),
                        path: path.to_path_buf(),
                        kind: rule.kind,
                        risk: rule.risk,
                    });
                    walker.skip_current_dir();
                    continue;
                }
                let name = entry.file_name().to_string_lossy();
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
    fn venv_requires_pyvenv_cfg_inside() {
        let dir = tempfile::tempdir().unwrap();
        let venv = dir.path().join(".venv");
        std::fs::create_dir_all(&venv).unwrap();
        assert!(matching_rule(&venv).is_none());
        std::fs::write(venv.join("pyvenv.cfg"), "").unwrap();
        assert_eq!(matching_rule(&venv).unwrap().risk, RiskLevel::Medium);
    }
}
