//! [`PruneEngine`] ties platform, safety, providers and deletion together.
//! It is stateless with respect to sessions; the caller (desktop app, CLI) stores
//! [`ScanSession`]s and [`CleanupPlan`]s and passes them back in.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use chrono::Utc;

use crate::fs;
use crate::models::{
    BlockedTarget, CleanupPlan, CleanupProgress, CleanupResult, CleanupStatus, CleanupTarget,
    DeleteMode, FailedTarget, OperationRecord, ProviderInfo, RiskLevel, ScanProgress, ScanSession,
    TargetKind,
};
use crate::ops::OperationLog;
use crate::platform::{self, KnownPaths, PlatformService};
use crate::providers::ProviderRegistry;
use crate::safety::SafetyPolicy;
use crate::scan::{ScanRequest, Scanner};
use crate::Result;

pub struct PruneEngine {
    platform: Box<dyn PlatformService>,
    known: KnownPaths,
    policy: SafetyPolicy,
    registry: ProviderRegistry,
}

impl PruneEngine {
    /// Engine for the running OS with the built-in providers.
    pub fn new() -> Self {
        Self::with_parts(platform::current(), ProviderRegistry::with_defaults())
    }

    /// Engine with a custom platform (tests, sandboxes) and provider set.
    pub fn with_parts(platform: Box<dyn PlatformService>, registry: ProviderRegistry) -> Self {
        let known = platform.known_paths();
        let policy = SafetyPolicy::for_platform(platform.as_ref(), &known);
        Self {
            platform,
            known,
            policy,
            registry,
        }
    }

    pub fn platform(&self) -> &dyn PlatformService {
        self.platform.as_ref()
    }

    pub fn known_paths(&self) -> &KnownPaths {
        &self.known
    }

    pub fn policy(&self) -> &SafetyPolicy {
        &self.policy
    }

    pub fn registry(&self) -> &ProviderRegistry {
        &self.registry
    }

    pub fn providers(&self) -> Vec<ProviderInfo> {
        self.registry.infos(&self.known)
    }

    /// Blocking scan; run on a worker thread.
    pub fn scan(
        &self,
        scan_id: &str,
        request: &ScanRequest,
        cancel: Arc<AtomicBool>,
        on_progress: &(dyn Fn(ScanProgress) + Send + Sync),
    ) -> ScanSession {
        Scanner {
            known: &self.known,
            policy: &self.policy,
            registry: &self.registry,
        }
        .run(scan_id, request, cancel, on_progress)
    }

    /// Dry run. Resolves target ids against the session, validates every path, and reports what
    /// would be removed. Touches nothing.
    pub fn plan(
        &self,
        session: &ScanSession,
        target_ids: &[String],
        mode: DeleteMode,
    ) -> CleanupPlan {
        let mut targets: Vec<CleanupTarget> = Vec::new();
        let mut blocked: Vec<BlockedTarget> = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for id in target_ids {
            if !seen.insert(id.as_str()) {
                continue;
            }
            let Some(target) = session.find_target(id) else {
                blocked.push(BlockedTarget {
                    target_id: id.clone(),
                    path: String::new(),
                    reason: "not part of this scan".into(),
                });
                continue;
            };
            if target.risk == RiskLevel::Protected {
                blocked.push(BlockedTarget {
                    target_id: id.clone(),
                    path: target.path.clone(),
                    reason: "protected".into(),
                });
                continue;
            }
            match self.policy.validate(&target.path_buf()) {
                Ok(_) => targets.push(target.clone()),
                Err(e) => blocked.push(BlockedTarget {
                    target_id: id.clone(),
                    path: target.path.clone(),
                    reason: e.to_string(),
                }),
            }
        }

        let total_bytes = targets.iter().map(|t| t.size_bytes).sum();
        let file_count = targets.iter().map(|t| t.file_count).sum();
        let directory_count = targets
            .iter()
            .filter(|t| t.kind == TargetKind::Directory)
            .count() as u64;

        CleanupPlan {
            id: uuid::Uuid::new_v4().to_string(),
            scan_id: session.id.clone(),
            mode,
            created_at: Utc::now(),
            targets,
            blocked,
            total_bytes,
            file_count,
            directory_count,
        }
    }

    /// Execute a plan. Every path is validated *again* immediately before removal.
    /// Targets flagged `permanent_only` are deleted permanently regardless of `plan.mode`.
    pub fn execute(
        &self,
        plan: &CleanupPlan,
        log: Option<&OperationLog>,
        on_progress: &(dyn Fn(CleanupProgress) + Send + Sync),
    ) -> Result<CleanupResult> {
        let started_at = Utc::now();
        let total = plan.targets.len();
        let mut removed_targets = 0usize;
        let mut removed_files = 0u64;
        let mut removed_bytes = 0u64;
        let mut failed: Vec<FailedTarget> = Vec::new();

        for (i, target) in plan.targets.iter().enumerate() {
            let path = target.path_buf();
            let mode = if target.permanent_only {
                DeleteMode::Permanent
            } else {
                plan.mode
            };
            let outcome = self
                .policy
                .validate(&path)
                .and_then(|validated| fs::remove(&validated, mode));
            match outcome {
                Ok(_) => {
                    removed_targets += 1;
                    removed_files += target.file_count;
                    removed_bytes += target.size_bytes;
                    tracing::info!(path = %target.path, ?mode, "removed");
                }
                Err(e) => {
                    tracing::warn!(path = %target.path, error = %e, "failed to remove");
                    failed.push(FailedTarget {
                        target_id: target.id.clone(),
                        path: target.path.clone(),
                        error: e.to_string(),
                    });
                }
            }
            on_progress(CleanupProgress {
                plan_id: plan.id.clone(),
                done: i + 1,
                total,
                current_path: target.path.clone(),
                removed_bytes,
            });
        }

        let status = match (removed_targets, failed.len()) {
            (_, 0) => CleanupStatus::Success,
            (0, _) => CleanupStatus::Failed,
            _ => CleanupStatus::Partial,
        };
        let result = CleanupResult {
            operation_id: uuid::Uuid::new_v4().to_string(),
            plan_id: plan.id.clone(),
            mode: plan.mode,
            status,
            started_at,
            finished_at: Utc::now(),
            removed_targets,
            removed_files,
            removed_bytes,
            failed,
        };

        if let Some(log) = log {
            let mut providers: Vec<String> =
                plan.targets.iter().map(|t| t.provider_id.clone()).collect();
            providers.sort();
            providers.dedup();
            let title = self.title_for(&providers);
            log.append(&OperationRecord {
                id: result.operation_id.clone(),
                at: result.finished_at,
                title,
                mode: plan.mode,
                status,
                removed_targets,
                removed_files,
                removed_bytes,
                failed_count: result.failed.len(),
                providers,
            })?;
        }
        Ok(result)
    }

    fn title_for(&self, providers: &[String]) -> String {
        match providers {
            [] => "Cleanup".to_string(),
            [one] => format!(
                "Clean {}",
                self.registry.get(one).map(|p| p.name()).unwrap_or(one)
            ),
            many => format!("Clean {} categories", many.len()),
        }
    }
}

impl Default for PruneEngine {
    fn default() -> Self {
        Self::new()
    }
}
