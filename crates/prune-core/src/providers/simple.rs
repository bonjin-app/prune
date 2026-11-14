use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::{CleanupProvider, ScanContext, ScanOutput};
use crate::models::{Category, RiskLevel};
use crate::platform::{InUse, KnownPaths};

/// Shown on an item a running program is using, when the OS does not say which program.
pub const IN_USE: &str = "In use by a running program";

/// How a root directory is turned into targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootMode {
    /// The root itself is one target (e.g. `~/.npm/_cacache`).
    Whole,
    /// Every direct child of the root is a target (e.g. each app under `~/Library/Caches`).
    Children,
}

/// A root directory for a [`SimpleProvider`].
#[derive(Debug, Clone)]
pub struct Root {
    pub path: PathBuf,
    pub mode: RootMode,
    /// Display label for `Whole` roots. Defaults to the last path component.
    pub label: Option<String>,
}

impl Root {
    pub fn whole(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            mode: RootMode::Whole,
            label: None,
        }
    }

    pub fn labelled(path: impl Into<PathBuf>, label: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            mode: RootMode::Whole,
            label: Some(label.into()),
        }
    }

    pub fn children(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            mode: RootMode::Children,
            label: None,
        }
    }
}

/// Declarative provider: a set of roots resolved from [`KnownPaths`] plus a few filters.
/// Covers the vast majority of cache locations without custom code.
pub struct SimpleProvider {
    pub id: &'static str,
    pub name: &'static str,
    pub category: Category,
    pub description: &'static str,
    pub risk: RiskLevel,
    pub priority: u8,
    /// Roots are resolved lazily so missing tools simply yield no roots.
    pub roots: fn(&KnownPaths) -> Vec<Root>,
    /// Child names (case-sensitive) to skip in `Children` mode.
    pub exclude_names: &'static [&'static str],
    /// Child name prefixes (case-sensitive) to skip in `Children` mode.
    pub exclude_prefixes: &'static [&'static str],
    /// Only include entries not modified for at least this many days.
    pub min_age_days: Option<u32>,
    /// Targets that can only be deleted permanently (e.g. items already in the trash).
    pub permanent_only: bool,
    /// Ask the OS which candidates running programs are using, and report those as
    /// [`RiskLevel::Protected`] instead of offering them. For places where age alone does not
    /// say whether something is finished with, such as the temporary folder.
    pub protect_in_use: bool,
}

impl SimpleProvider {
    pub const fn new(
        id: &'static str,
        name: &'static str,
        category: Category,
        description: &'static str,
        risk: RiskLevel,
        roots: fn(&KnownPaths) -> Vec<Root>,
    ) -> Self {
        Self {
            id,
            name,
            category,
            description,
            risk,
            priority: 60,
            roots,
            exclude_names: &[],
            exclude_prefixes: &[],
            min_age_days: None,
            permanent_only: false,
            protect_in_use: false,
        }
    }

    pub const fn priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    pub const fn exclude(mut self, names: &'static [&'static str]) -> Self {
        self.exclude_names = names;
        self
    }

    pub const fn exclude_prefixed(mut self, prefixes: &'static [&'static str]) -> Self {
        self.exclude_prefixes = prefixes;
        self
    }

    pub const fn protect_in_use(mut self) -> Self {
        self.protect_in_use = true;
        self
    }

    pub const fn min_age_days(mut self, days: u32) -> Self {
        self.min_age_days = Some(days);
        self
    }

    pub const fn permanent_only(mut self) -> Self {
        self.permanent_only = true;
        self
    }

    fn excluded(&self, name: &str) -> bool {
        name == ".DS_Store"
            || self.exclude_names.contains(&name)
            || self.exclude_prefixes.iter().any(|p| name.starts_with(p))
    }

    fn old_enough(&self, path: &Path) -> bool {
        let Some(days) = self.min_age_days else {
            return true;
        };
        let Ok(meta) = std::fs::symlink_metadata(path) else {
            return false;
        };
        let Ok(modified) = meta.modified() else {
            return false;
        };
        let age = SystemTime::now()
            .duration_since(modified)
            .unwrap_or(Duration::ZERO);
        age >= Duration::from_secs(u64::from(days) * 86_400)
    }
}

pub fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

impl CleanupProvider for SimpleProvider {
    fn id(&self) -> &str {
        self.id
    }
    fn name(&self) -> &str {
        self.name
    }
    fn category(&self) -> Category {
        self.category
    }
    fn description(&self) -> &str {
        self.description
    }
    fn default_risk(&self) -> RiskLevel {
        self.risk
    }
    fn priority(&self) -> u8 {
        self.priority
    }

    fn is_available(&self, known: &KnownPaths) -> bool {
        (self.roots)(known).iter().any(|r| r.path.exists())
    }

    fn scan(&self, ctx: &ScanContext<'_>) -> ScanOutput {
        let mut out = ScanOutput::default();
        for root in (self.roots)(ctx.known) {
            if ctx.is_cancelled() {
                break;
            }
            if !root.path.exists() {
                continue;
            }
            let candidates: Vec<(PathBuf, String)> = match root.mode {
                RootMode::Whole => {
                    if !self.old_enough(&root.path) {
                        continue;
                    }
                    let label = root.label.clone().unwrap_or_else(|| file_name(&root.path));
                    vec![(root.path.clone(), label)]
                }
                RootMode::Children => {
                    let entries = match std::fs::read_dir(&root.path) {
                        Ok(e) => e,
                        Err(e) => {
                            out.issue(&root.path, e.to_string());
                            continue;
                        }
                    };
                    let mut children: Vec<PathBuf> =
                        entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
                    children.sort();
                    children
                        .into_iter()
                        .map(|child| {
                            let name = file_name(&child);
                            (child, name)
                        })
                        .filter(|(child, name)| !self.excluded(name) && self.old_enough(child))
                        .collect()
                }
            };
            if candidates.is_empty() || ctx.is_cancelled() {
                continue;
            }

            // Asking the OS can take a second; it runs while the candidates are measured
            // rather than before, and its answer is applied to what was measured.
            let (in_use, mut measured) = std::thread::scope(|scope| {
                let asking = self.protect_in_use.then(|| {
                    let paths: Vec<PathBuf> = candidates.iter().map(|(p, _)| p.clone()).collect();
                    scope.spawn(move || ctx.platform.in_use(&paths))
                });
                let mut measured = ScanOutput::default();
                for (path, label) in &candidates {
                    if ctx.is_cancelled() {
                        break;
                    }
                    measured.push_measured(
                        ctx,
                        self.id,
                        path,
                        label.clone(),
                        self.risk,
                        None,
                        self.permanent_only,
                    );
                }
                let in_use = asking.map(|handle| {
                    handle.join().unwrap_or_else(|_| {
                        InUse::Unknown("the check for files in use stopped unexpectedly".into())
                    })
                });
                (in_use, measured)
            });

            match in_use {
                None => {}
                Some(InUse::Known(found)) => {
                    for target in &mut measured.targets {
                        if let Some(program) = found.get(Path::new(&target.path)) {
                            target.risk = RiskLevel::Protected;
                            target.description = Some(match program {
                                Some(program) => format!("{IN_USE} ({program})"),
                                None => IN_USE.to_string(),
                            });
                        }
                    }
                }
                Some(InUse::Unknown(reason)) => {
                    // Failing open is only acceptable out loud: everything here is still
                    // offered, and the scan says why it could not check.
                    out.issue(
                        &root.path,
                        format!(
                            "Could not check which items running programs are using \
                             ({reason}), so none of them is marked as in use."
                        ),
                    );
                }
            }
            out.merge(measured);
        }
        out
    }
}
