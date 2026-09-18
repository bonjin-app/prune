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
    /// Other installed copies that answer to the same bundle identifier.
    ///
    /// Keeping `App-v1.2-backup.app` beside `App.app` is ordinary, and both copies read the
    /// same `Application Support`, `Caches` and `Preferences`, because those are keyed by
    /// bundle identifier rather than by which file was launched. Uninstalling the backup would
    /// then delete the settings of the copy still in use. When this is not empty the leftovers
    /// are reported as protected: they belong to no single copy, and once the others are gone a
    /// rescan finds nothing to share them with and offers them normally.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shared_with: Vec<String>,
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
    let shared_with = other_copies(platform, known, app);
    detail_knowing_siblings(platform, known, policy, app, cancel, shared_with)
}

/// Installed applications other than `app` that answer to the same bundle identifier.
///
/// Matched on the identifier, not the name: `PeterFan.app` and `PeterFan-v1.27.74-backup.app`
/// have different names and the same identifier, which is precisely the pair that shares data.
/// An application without an identifier matches nothing, since "no identifier" is not something
/// two applications can have in common.
fn other_copies(
    platform: &dyn PlatformService,
    known: &KnownPaths,
    app: &ApplicationInfo,
) -> Vec<String> {
    let Some(id) = app.bundle_id.as_deref().filter(|s| !s.is_empty()) else {
        return Vec::new();
    };
    let Ok(all) = platform.applications(known) else {
        return Vec::new();
    };
    all.into_iter()
        .filter(|other| other.id != app.id && other.bundle_id.as_deref() == Some(id))
        .map(|other| other.path)
        .filter(|path| !path.is_empty())
        .collect()
}

fn detail_knowing_siblings(
    platform: &dyn PlatformService,
    known: &KnownPaths,
    policy: &SafetyPolicy,
    app: &ApplicationInfo,
    cancel: &AtomicBool,
    shared_with: Vec<String>,
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
        let is_bundle = related.kind == AppDataKind::Application;
        let risk = if app.is_system && is_bundle {
            RiskLevel::Protected
        } else if !is_bundle && !shared_with.is_empty() {
            // Another copy is installed and reads the same data. This copy's bundle can still
            // go; its data belongs to both.
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
        shared_with,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::{sandbox::SandboxPlatform, ApplicationInfo, RelatedPath};
    use crate::safety::SafetyPolicy;

    /// Two copies of one application, as a machine with `App-v1-backup.app` beside `App.app`.
    struct TwoCopies {
        sandbox: SandboxPlatform,
        /// Whether the second copy is installed.
        backup_present: bool,
    }

    impl TwoCopies {
        fn app(&self, name: &str) -> ApplicationInfo {
            ApplicationInfo {
                id: format!("id-{name}"),
                name: name.to_string(),
                path: self
                    .sandbox
                    .home()
                    .join(format!("Applications/{name}.app"))
                    .to_string_lossy()
                    .into_owned(),
                version: None,
                bundle_id: Some("com.example.app".into()),
                publisher: None,
                size_bytes: None,
                modified_at: None,
                source: "applications".into(),
                is_system: false,
                uninstall_command: None,
            }
        }
    }

    impl PlatformService for TwoCopies {
        fn platform(&self) -> crate::models::Platform {
            self.sandbox.platform()
        }
        fn known_paths(&self) -> KnownPaths {
            self.sandbox.known_paths()
        }
        fn protected_paths(&self, known: &KnownPaths) -> crate::platform::ProtectedPaths {
            self.sandbox.protected_paths(known)
        }
        fn applications(&self, _known: &KnownPaths) -> crate::Result<Vec<ApplicationInfo>> {
            let mut all = vec![self.app("App")];
            if self.backup_present {
                all.push(self.app("App-backup"));
            }
            Ok(all)
        }
        fn app_related_paths(&self, app: &ApplicationInfo, known: &KnownPaths) -> Vec<RelatedPath> {
            // Both copies answer to the same identifier, so both are pointed at the same data.
            vec![
                RelatedPath {
                    kind: AppDataKind::Application,
                    path: std::path::PathBuf::from(&app.path),
                },
                RelatedPath {
                    kind: AppDataKind::ApplicationSupport,
                    path: known.app_support.clone().unwrap().join("App"),
                },
            ]
        }
    }

    fn fixture(backup_present: bool) -> (tempfile::TempDir, TwoCopies) {
        let dir = tempfile::tempdir().unwrap();
        let sandbox = SandboxPlatform::new(dir.path());
        sandbox.prepare().unwrap();
        for name in ["App.app", "App-backup.app"] {
            std::fs::create_dir_all(sandbox.home().join("Applications").join(name)).unwrap();
            std::fs::write(
                sandbox.home().join("Applications").join(name).join("bin"),
                b"xxxx",
            )
            .unwrap();
        }
        std::fs::create_dir_all(sandbox.home().join("Library/Application Support/App")).unwrap();
        std::fs::write(
            sandbox
                .home()
                .join("Library/Application Support/App/settings"),
            b"the settings both copies read",
        )
        .unwrap();
        (
            dir,
            TwoCopies {
                sandbox,
                backup_present,
            },
        )
    }

    #[test]
    fn a_second_copy_makes_the_shared_data_untouchable() {
        let (_dir, platform) = fixture(true);
        let known = platform.known_paths();
        let policy = SafetyPolicy::for_platform(&platform, &known);
        let cancel = AtomicBool::new(false);

        let detail = detail(
            &platform,
            &known,
            &policy,
            &platform.app("App-backup"),
            &cancel,
        );

        assert_eq!(
            detail.shared_with.len(),
            1,
            "the other copy should be named: {:?}",
            detail.shared_with
        );
        assert!(detail.shared_with[0].ends_with("App.app"));

        let support = detail
            .items
            .iter()
            .find(|i| i.kind == AppDataKind::ApplicationSupport)
            .expect("the shared data should still be listed, not hidden");
        assert_eq!(
            support.target.risk,
            RiskLevel::Protected,
            "removing the backup must not take the working copy's settings"
        );

        // The copy the user actually asked to remove can still go.
        let bundle = detail
            .items
            .iter()
            .find(|i| i.kind == AppDataKind::Application)
            .unwrap();
        assert_ne!(bundle.target.risk, RiskLevel::Protected);
    }

    #[test]
    fn the_last_copy_can_take_its_data_with_it() {
        let (_dir, platform) = fixture(false);
        let known = platform.known_paths();
        let policy = SafetyPolicy::for_platform(&platform, &known);
        let cancel = AtomicBool::new(false);

        let detail = detail(&platform, &known, &policy, &platform.app("App"), &cancel);

        assert!(detail.shared_with.is_empty());
        let support = detail
            .items
            .iter()
            .find(|i| i.kind == AppDataKind::ApplicationSupport)
            .unwrap();
        assert_ne!(
            support.target.risk,
            RiskLevel::Protected,
            "with nothing else reading it, the data is the application's to take"
        );
    }
}
