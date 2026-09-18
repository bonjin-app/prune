use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::{CleanupProvider, ScanContext, ScanOutput};
use crate::models::{Category, RiskLevel};
use crate::platform::KnownPaths;

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
    /// Only include entries not modified for at least this many days.
    pub min_age_days: Option<u32>,
    /// Targets that can only be deleted permanently (e.g. items already in the trash).
    pub permanent_only: bool,
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
            min_age_days: None,
            permanent_only: false,
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

    pub const fn min_age_days(mut self, days: u32) -> Self {
        self.min_age_days = Some(days);
        self
    }

    pub const fn permanent_only(mut self) -> Self {
        self.permanent_only = true;
        self
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
            match root.mode {
                RootMode::Whole => {
                    if !self.old_enough(&root.path) {
                        continue;
                    }
                    let label = root.label.clone().unwrap_or_else(|| file_name(&root.path));
                    out.push_measured(
                        ctx,
                        self.id,
                        &root.path,
                        label,
                        self.risk,
                        None,
                        self.permanent_only,
                    );
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
                    for child in children {
                        if ctx.is_cancelled() {
                            break;
                        }
                        let name = file_name(&child);
                        if name == ".DS_Store" || self.exclude_names.contains(&name.as_str()) {
                            continue;
                        }
                        if !self.old_enough(&child) {
                            continue;
                        }
                        out.push_measured(
                            ctx,
                            self.id,
                            &child,
                            name,
                            self.risk,
                            None,
                            self.permanent_only,
                        );
                    }
                }
            }
        }
        out
    }
}
