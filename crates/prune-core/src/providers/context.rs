use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, Utc};

use crate::fs::{entry_stats, SizeContext};
use crate::models::{CleanupTarget, RiskLevel, ScanIssue, TargetKind};
use crate::platform::KnownPaths;
use crate::safety::SafetyPolicy;

/// Everything a provider needs while scanning.
pub struct ScanContext<'a> {
    pub known: &'a KnownPaths,
    pub policy: &'a SafetyPolicy,
    pub cancel: &'a AtomicBool,
    /// `(files_visited, bytes_found, current_path)` — call freely, the engine throttles.
    pub progress: &'a (dyn Fn(u64, u64, &Path) + Send + Sync),
}

impl<'a> ScanContext<'a> {
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

/// One measured candidate: the target if it turned out to be worth reporting, plus anything
/// that went wrong while measuring it.
#[derive(Debug, Default)]
pub struct Measured {
    pub target: Option<CleanupTarget>,
    pub issues: Vec<ScanIssue>,
}

/// Measures one candidate path without touching any shared state.
///
/// Providers that discover many candidates (the project artifact walker finds hundreds) measure
/// them in parallel, so this deliberately owns everything it returns rather than writing into a
/// [`ScanOutput`].
#[allow(clippy::too_many_arguments)]
pub fn measure(
    ctx: &ScanContext<'_>,
    provider_id: &str,
    path: &Path,
    label: impl Into<String>,
    risk: RiskLevel,
    description: Option<String>,
    permanent_only: bool,
) -> Measured {
    let mut issues = Vec::new();
    let meta = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) => {
            issues.push(ScanIssue {
                path: path.to_string_lossy().into(),
                message: e.to_string(),
            });
            return Measured {
                target: None,
                issues,
            };
        }
    };
    let kind = if meta.is_dir() {
        TargetKind::Directory
    } else {
        TargetKind::File
    };
    let mut size_ctx = SizeContext {
        cancel: ctx.cancel,
        on_progress: Some(ctx.progress),
        issues: &mut issues,
    };
    let stats = entry_stats(path, &mut size_ctx);
    (ctx.progress)(stats.files, stats.bytes, path);
    if stats.bytes == 0 && stats.files == 0 {
        return Measured {
            target: None,
            issues,
        };
    }
    let risk = if ctx.policy.is_protected(path) {
        RiskLevel::Protected
    } else {
        risk
    };
    Measured {
        target: Some(CleanupTarget {
            id: CleanupTarget::make_id(provider_id, path),
            provider_id: provider_id.to_string(),
            path: path.to_string_lossy().into_owned(),
            kind,
            size_bytes: stats.bytes,
            file_count: stats.files,
            risk,
            label: label.into(),
            description,
            modified_at: meta.modified().ok().map(DateTime::<Utc>::from),
            permanent_only,
        }),
        issues,
    }
}

/// Targets plus non-fatal issues from one provider run.
#[derive(Debug, Default)]
pub struct ScanOutput {
    pub targets: Vec<CleanupTarget>,
    pub issues: Vec<ScanIssue>,
}

impl ScanOutput {
    pub fn issue(&mut self, path: &Path, message: impl Into<String>) {
        self.issues.push(ScanIssue {
            path: path.to_string_lossy().into(),
            message: message.into(),
        });
    }

    /// Measure `path` and append it as a target. Empty entries are skipped.
    ///
    /// The risk is escalated to [`RiskLevel::Protected`] when the safety policy refuses the
    /// path, so protected items are still *visible* but can never be selected.
    #[allow(clippy::too_many_arguments)]
    pub fn push_measured(
        &mut self,
        ctx: &ScanContext<'_>,
        provider_id: &str,
        path: &Path,
        label: impl Into<String>,
        risk: RiskLevel,
        description: Option<String>,
        permanent_only: bool,
    ) -> Option<&CleanupTarget> {
        let measured = measure(
            ctx,
            provider_id,
            path,
            label,
            risk,
            description,
            permanent_only,
        );
        self.issues.extend(measured.issues);
        let target = measured.target?;
        self.targets.push(target);
        self.targets.last()
    }

    pub fn merge(&mut self, other: ScanOutput) {
        self.targets.extend(other.targets);
        self.issues.extend(other.issues);
    }
}
