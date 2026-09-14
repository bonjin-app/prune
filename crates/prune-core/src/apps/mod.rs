//! Uninstaller support: list applications, measure them, and turn an application plus its
//! leftover data into [`CleanupTarget`]s that flow through the normal safety pipeline.

use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};

use crate::fs::{entry_stats, SizeContext};
use crate::models::{Category, CleanupTarget, RiskLevel, ScanResult};
use crate::platform::{AppDataKind, ApplicationInfo, KnownPaths, PlatformService};
use crate::providers::{ScanContext, ScanOutput};
use crate::safety::SafetyPolicy;

/// Provider id for uninstaller targets.
pub const UNINSTALLER_PROVIDER: &str = "uninstaller";

/// One piece of an application: the bundle itself or a leftover data location.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppDataItem {
    pub kind: AppDataKind,
    pub kind_label: String,
    pub target: CleanupTarget,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppDetail {
    pub app: ApplicationInfo,
    pub items: Vec<AppDataItem>,
    /// Bundle + all leftovers.
    pub total_bytes: u64,
    /// Leftovers only (what a plain drag-to-Trash would leave behind).
    pub leftover_bytes: u64,
}

/// Progress event while application sizes are measured.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppsProgress {
    pub scan_id: String,
    pub done: usize,
    pub total: usize,
    pub app: ApplicationInfo,
}

/// List applications (no sizes). Cheap.
pub fn list(
    platform: &dyn PlatformService,
    known: &KnownPaths,
) -> crate::Result<Vec<ApplicationInfo>> {
    platform.applications(known)
}

/// Measure the size of an application's bundle / install directory.
pub fn measure(app: &ApplicationInfo, cancel: &AtomicBool) -> Option<u64> {
    if app.path.is_empty() {
        return app.size_bytes;
    }
    let mut issues = Vec::new();
    let mut ctx = SizeContext {
        cancel,
        on_progress: None,
        issues: &mut issues,
    };
    Some(entry_stats(std::path::Path::new(&app.path), &mut ctx).bytes)
}

/// Application plus leftovers, measured, with risk classified by the policy.
pub fn detail(
    platform: &dyn PlatformService,
    known: &KnownPaths,
    policy: &SafetyPolicy,
    app: &ApplicationInfo,
    cancel: &AtomicBool,
) -> AppDetail {
    let noop = |_: u64, _: u64, _: &std::path::Path| {};
    let ctx = ScanContext {
        known,
        policy,
        cancel,
        progress: &noop,
    };
    let mut out = ScanOutput::default();
    let mut items = Vec::new();

    for related in platform.app_related_paths(app, known) {
        let risk = if app.is_system && related.kind == AppDataKind::Application {
            RiskLevel::Protected
        } else {
            related.kind.risk()
        };
        let label = match related.kind {
            AppDataKind::Application => app.name.clone(),
            _ => related
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| related.kind.label().to_string()),
        };
        let before = out.targets.len();
        out.push_measured(
            &ctx,
            UNINSTALLER_PROVIDER,
            &related.path,
            label,
            risk,
            Some(related.kind.label().to_string()),
            false,
        );
        if out.targets.len() > before {
            let mut target = out.targets[before].clone();
            if risk == RiskLevel::Protected {
                target.risk = RiskLevel::Protected;
            }
            items.push(AppDataItem {
                kind: related.kind,
                kind_label: related.kind.label().to_string(),
                target,
            });
        }
    }

    let total_bytes = items.iter().map(|i| i.target.size_bytes).sum();
    let leftover_bytes = items
        .iter()
        .filter(|i| i.kind != AppDataKind::Application)
        .map(|i| i.target.size_bytes)
        .sum();
    let mut app = app.clone();
    if let Some(bundle) = items.iter().find(|i| i.kind == AppDataKind::Application) {
        app.size_bytes = Some(bundle.target.size_bytes);
    }
    AppDetail {
        app,
        items,
        total_bytes,
        leftover_bytes,
    }
}

/// Synthetic scan result so the detail's targets can be previewed and executed.
pub fn as_scan_result(detail: &AppDetail) -> ScanResult {
    let mut r = ScanResult {
        provider_id: UNINSTALLER_PROVIDER.into(),
        provider_name: format!("Uninstall {}", detail.app.name),
        category: Category::Applications,
        targets: detail.items.iter().map(|i| i.target.clone()).collect(),
        total_bytes: 0,
        total_files: 0,
        duration_ms: 0,
        issues: Vec::new(),
    };
    r.recompute_totals();
    r
}
