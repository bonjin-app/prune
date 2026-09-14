//! Runs providers in parallel, aggregates results and removes overlapping targets.

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use chrono::Utc;
use rayon::prelude::*;

use crate::models::{CleanupTarget, ScanProgress, ScanResult, ScanSession, ScanStatus};
use crate::platform::KnownPaths;
use crate::providers::{CleanupProvider, ProviderRegistry, ScanContext};
use crate::safety::SafetyPolicy;

/// Which providers to run. `None` = every available provider.
#[derive(Debug, Clone, Default)]
pub struct ScanRequest {
    pub provider_ids: Option<Vec<String>>,
}

/// Runs a scan. Cheap to construct; holds only references.
pub struct Scanner<'a> {
    pub known: &'a KnownPaths,
    pub policy: &'a SafetyPolicy,
    pub registry: &'a ProviderRegistry,
}

impl<'a> Scanner<'a> {
    /// Blocking. Call from a worker thread. `on_progress` may be invoked from several threads.
    pub fn run(
        &self,
        scan_id: &str,
        request: &ScanRequest,
        cancel: Arc<AtomicBool>,
        on_progress: &(dyn Fn(ScanProgress) + Send + Sync),
    ) -> ScanSession {
        let started_at = Utc::now();
        let providers: Vec<Arc<dyn CleanupProvider>> = self
            .registry
            .all()
            .iter()
            .filter(|p| match &request.provider_ids {
                Some(ids) => ids.iter().any(|id| id == p.id()),
                None => true,
            })
            .filter(|p| p.is_available(self.known))
            .cloned()
            .collect();

        let total = providers.len();
        let done = AtomicUsize::new(0);
        let results: Mutex<Vec<ScanResult>> = Mutex::new(Vec::with_capacity(total));

        providers.par_iter().for_each(|provider| {
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            let start = Instant::now();
            let last_emit = Mutex::new(Instant::now());
            let progress = |files: u64, bytes: u64, path: &Path| {
                // Throttle to ~10 events / second per provider.
                let mut last = last_emit.lock().unwrap();
                if last.elapsed().as_millis() < 100 {
                    return;
                }
                *last = Instant::now();
                on_progress(ScanProgress {
                    scan_id: scan_id.to_string(),
                    provider_id: provider.id().to_string(),
                    scanned_files: files,
                    discovered_bytes: bytes,
                    current_path: Some(path.to_string_lossy().into_owned()),
                    providers_done: done.load(Ordering::Relaxed),
                    providers_total: total,
                });
            };
            let ctx = ScanContext {
                known: self.known,
                policy: self.policy,
                cancel: &cancel,
                progress: &progress,
            };
            let output = provider.scan(&ctx);
            let mut result = ScanResult {
                provider_id: provider.id().to_string(),
                provider_name: provider.name().to_string(),
                category: provider.category(),
                targets: output.targets,
                total_bytes: 0,
                total_files: 0,
                duration_ms: start.elapsed().as_millis() as u64,
                issues: output.issues,
            };
            result.recompute_totals();
            let finished = done.fetch_add(1, Ordering::Relaxed) + 1;
            on_progress(ScanProgress {
                scan_id: scan_id.to_string(),
                provider_id: provider.id().to_string(),
                scanned_files: result.total_files,
                discovered_bytes: result.total_bytes,
                current_path: None,
                providers_done: finished,
                providers_total: total,
            });
            results.lock().unwrap().push(result);
        });

        let mut results = results.into_inner().unwrap();
        // Keep registry order for a stable UI.
        let order: Vec<&str> = self.registry.all().iter().map(|p| p.id()).collect();
        results.sort_by_key(|r| {
            order
                .iter()
                .position(|id| *id == r.provider_id)
                .unwrap_or(usize::MAX)
        });

        dedupe_overlaps(&mut results, self.registry);

        let mut session = ScanSession {
            id: scan_id.to_string(),
            status: if cancel.load(Ordering::Relaxed) {
                ScanStatus::Cancelled
            } else {
                ScanStatus::Completed
            },
            started_at,
            finished_at: Some(Utc::now()),
            results,
            total_bytes: 0,
            total_files: 0,
        };
        session.recompute_totals();
        session
    }
}

/// When two targets overlap (same path, or one contains the other), keep the one from the
/// higher-priority provider. On equal priority keep the ancestor (it already includes the
/// descendant). This lets a generic "all caches" provider coexist with specialised ones.
pub fn dedupe_overlaps(results: &mut [ScanResult], registry: &ProviderRegistry) {
    let priority = |provider_id: &str| {
        registry
            .get(provider_id)
            .map(|p| p.priority())
            .unwrap_or(50)
    };

    // (path, provider priority, result index, target index)
    let mut all: Vec<(std::path::PathBuf, u8, usize, usize)> = Vec::new();
    for (ri, r) in results.iter().enumerate() {
        let prio = priority(&r.provider_id);
        for (ti, t) in r.targets.iter().enumerate() {
            all.push((t.path_buf(), prio, ri, ti));
        }
    }
    all.sort_by(|a, b| a.0.cmp(&b.0));

    let mut drop: Vec<(usize, usize)> = Vec::new();
    for i in 0..all.len() {
        let (ref path_i, prio_i, ri_i, ti_i) = all[i];
        for (path_j, prio_j, ri_j, ti_j) in all.iter().skip(i + 1).map(|e| (&e.0, e.1, e.2, e.3)) {
            if !path_j.starts_with(path_i) {
                break; // sorted: no later entry can be inside path_i
            }
            if ri_i == ri_j && *path_i == *path_j {
                continue;
            }
            // path_j is equal to or inside path_i.
            let drop_i = prio_j > prio_i || (prio_j == prio_i && *path_i == *path_j && ri_j < ri_i);
            if drop_i {
                drop.push((ri_i, ti_i));
            } else {
                drop.push((ri_j, ti_j));
            }
        }
    }
    drop.sort_unstable();
    drop.dedup();
    for (ri, r) in results.iter_mut().enumerate() {
        let mut idx = 0usize;
        r.targets.retain(|_| {
            let keep = !drop.contains(&(ri, idx));
            idx += 1;
            keep
        });
        r.recompute_totals();
    }
}

/// Convenience: all targets across results.
pub fn all_targets(session: &ScanSession) -> impl Iterator<Item = &CleanupTarget> {
    session.results.iter().flat_map(|r| r.targets.iter())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Category, RiskLevel, TargetKind};
    use crate::providers::SimpleProvider;

    const GENERIC: SimpleProvider = SimpleProvider::new(
        "generic",
        "Generic",
        Category::ApplicationCache,
        "",
        RiskLevel::Safe,
        |_| vec![],
    )
    .priority(10);
    const SPECIFIC: SimpleProvider = SimpleProvider::new(
        "specific",
        "Specific",
        Category::DeveloperFiles,
        "",
        RiskLevel::Safe,
        |_| vec![],
    )
    .priority(60);

    fn target(provider: &str, path: &str, bytes: u64) -> CleanupTarget {
        CleanupTarget {
            id: CleanupTarget::make_id(provider, Path::new(path)),
            provider_id: provider.into(),
            path: path.into(),
            kind: TargetKind::Directory,
            size_bytes: bytes,
            file_count: 1,
            risk: RiskLevel::Safe,
            label: path.into(),
            description: None,
            modified_at: None,
            permanent_only: false,
        }
    }

    fn result(provider: &str, category: Category, targets: Vec<CleanupTarget>) -> ScanResult {
        let mut r = ScanResult {
            provider_id: provider.into(),
            provider_name: provider.into(),
            category,
            targets,
            total_bytes: 0,
            total_files: 0,
            duration_ms: 0,
            issues: vec![],
        };
        r.recompute_totals();
        r
    }

    #[test]
    fn specialised_provider_wins_over_generic_ancestor() {
        let registry = ProviderRegistry::new(vec![Arc::new(GENERIC), Arc::new(SPECIFIC)]);
        let mut results = vec![
            result(
                "generic",
                Category::ApplicationCache,
                vec![
                    target("generic", "/home/u/Library/Caches/Homebrew", 100),
                    target("generic", "/home/u/Library/Caches/Other", 5),
                ],
            ),
            result(
                "specific",
                Category::DeveloperFiles,
                vec![target("specific", "/home/u/Library/Caches/Homebrew", 100)],
            ),
        ];
        dedupe_overlaps(&mut results, &registry);
        assert_eq!(results[0].targets.len(), 1);
        assert_eq!(results[0].targets[0].path, "/home/u/Library/Caches/Other");
        assert_eq!(results[1].targets.len(), 1);
    }

    #[test]
    fn descendant_of_specialised_target_drops_generic_ancestor() {
        let registry = ProviderRegistry::new(vec![Arc::new(GENERIC), Arc::new(SPECIFIC)]);
        let mut results = vec![
            result(
                "generic",
                Category::ApplicationCache,
                vec![target("generic", "/c/Google", 100)],
            ),
            result(
                "specific",
                Category::DeveloperFiles,
                vec![target("specific", "/c/Google/Chrome/Default/Cache", 40)],
            ),
        ];
        dedupe_overlaps(&mut results, &registry);
        assert!(results[0].targets.is_empty());
        assert_eq!(results[1].targets.len(), 1);
    }

    #[test]
    fn same_priority_keeps_ancestor() {
        let registry = ProviderRegistry::new(vec![Arc::new(SPECIFIC)]);
        let mut results = vec![result(
            "specific",
            Category::DeveloperFiles,
            vec![
                target("specific", "/p/app/node_modules", 100),
                target("specific", "/p/app/node_modules/x/dist", 10),
            ],
        )];
        dedupe_overlaps(&mut results, &registry);
        assert_eq!(results[0].targets.len(), 1);
        assert_eq!(results[0].targets[0].path, "/p/app/node_modules");
    }
}
