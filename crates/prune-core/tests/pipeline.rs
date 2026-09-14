//! End-to-end test of the safety pipeline against a sandboxed fake home directory.
//! Never touches the real filesystem outside a temp dir.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use prune_core::models::{DeleteMode, Platform, RiskLevel};
use prune_core::ops::OperationLog;
use prune_core::platform::{KnownPaths, PlatformService, ProtectedPaths};
use prune_core::providers::ProviderRegistry;
use prune_core::scan::ScanRequest;
use prune_core::PruneEngine;

struct SandboxPlatform {
    root: PathBuf,
}

impl PlatformService for SandboxPlatform {
    fn platform(&self) -> Platform {
        Platform::current()
    }

    fn known_paths(&self) -> KnownPaths {
        let home = self.root.join("home");
        KnownPaths {
            user_cache: Some(home.join("Library/Caches")),
            user_logs: Some(home.join("Library/Logs")),
            app_support: Some(home.join("Library/Application Support")),
            local_app_data: Some(home.join("Library/Application Support")),
            temp: self.root.join("tmp"),
            trash: Some(home.join(".Trash")),
            downloads: Some(home.join("Downloads")),
            project_roots: vec![home.join("Projects")],
            home,
        }
    }

    fn protected_paths(&self, known: &KnownPaths) -> ProtectedPaths {
        let home = &known.home;
        ProtectedPaths {
            allowed_roots: vec![home.clone(), known.temp.clone()],
            exact: vec![
                home.clone(),
                home.join("Library"),
                home.join("Library/Caches"),
                home.join("Documents"),
            ],
            trees: vec![home.join(".ssh"), home.join("Library/Keychains")],
        }
    }
}

fn write(path: &Path, size: usize) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, vec![b'x'; size]).unwrap();
}

fn build_sandbox() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    // caches (safe)
    write(&home.join("Library/Caches/com.example.app/blob.bin"), 4096);
    write(&home.join("Library/Caches/Homebrew/pkg.tar.gz"), 2048);
    // logs
    write(&home.join("Library/Logs/app.log"), 512);
    // npm cache (specialised provider)
    write(&home.join(".npm/_cacache/index/x"), 1024);
    // project with node_modules + rust target
    write(&home.join("Projects/web/package.json"), 2);
    write(
        &home.join("Projects/web/node_modules/left-pad/index.js"),
        300,
    );
    write(&home.join("Projects/web/src/index.ts"), 50);
    write(&home.join("Projects/cli/Cargo.toml"), 2);
    write(&home.join("Projects/cli/target/debug/cli"), 700);
    // a folder called node_modules WITHOUT package.json must be ignored
    write(&home.join("Projects/notes/node_modules/readme.txt"), 10);
    // important data that must never be reported or removed
    write(&home.join("Documents/thesis.txt"), 100);
    write(&home.join(".ssh/id_ed25519"), 64);
    // trash
    write(&home.join(".Trash/old.txt"), 128);
    std::fs::create_dir_all(dir.path().join("tmp")).unwrap();
    dir
}

fn engine_for(dir: &tempfile::TempDir) -> PruneEngine {
    PruneEngine::with_parts(
        Box::new(SandboxPlatform {
            root: dir.path().to_path_buf(),
        }),
        ProviderRegistry::with_defaults(),
    )
}

#[test]
fn scan_discovers_expected_targets_and_dedupes_overlaps() {
    let dir = build_sandbox();
    let engine = engine_for(&dir);
    let session = engine.scan(
        "s1",
        &ScanRequest::default(),
        Arc::new(AtomicBool::new(false)),
        &|_| {},
    );

    let paths: Vec<(String, String)> = session
        .results
        .iter()
        .flat_map(|r| {
            r.targets
                .iter()
                .map(|t| (r.provider_id.clone(), t.path.clone()))
        })
        .collect();
    let has = |provider: &str, suffix: &str| {
        paths
            .iter()
            .any(|(p, path)| p == provider && path.ends_with(suffix))
    };

    assert!(has("user_cache", "Library/Caches/com.example.app"));
    assert!(has("homebrew_cache", "Library/Caches/Homebrew"));
    // The generic cache provider must not also report Homebrew.
    assert!(!has("user_cache", "Library/Caches/Homebrew"));
    assert!(has("user_logs", "Library/Logs/app.log"));
    assert!(has("npm_cache", ".npm/_cacache"));
    assert!(has("project_artifacts", "Projects/web/node_modules"));
    assert!(has("project_artifacts", "Projects/cli/target"));
    assert!(!has("project_artifacts", "Projects/notes/node_modules"));
    assert!(has("trash", ".Trash/old.txt"));
    // Nothing from protected places.
    assert!(!paths
        .iter()
        .any(|(_, p)| p.contains("Documents") || p.contains(".ssh")));

    let node_modules = session
        .results
        .iter()
        .flat_map(|r| r.targets.iter())
        .find(|t| t.path.ends_with("Projects/web/node_modules"))
        .unwrap();
    assert_eq!(node_modules.size_bytes, 300);
    assert_eq!(node_modules.file_count, 1);
    assert_eq!(node_modules.risk, RiskLevel::Low);
    assert!(session.total_bytes >= 4096 + 2048 + 512 + 1024 + 300 + 700 + 128);
}

#[test]
fn plan_blocks_unknown_ids_and_execute_removes_only_planned_targets() {
    let dir = build_sandbox();
    let engine = engine_for(&dir);
    let session = engine.scan(
        "s2",
        &ScanRequest::default(),
        Arc::new(AtomicBool::new(false)),
        &|_| {},
    );
    let home = dir.path().join("home");

    let all_ids: Vec<String> = session
        .results
        .iter()
        .flat_map(|r| r.targets.iter().map(|t| t.id.clone()))
        .collect();
    let cache_id = session
        .results
        .iter()
        .flat_map(|r| r.targets.iter())
        .find(|t| t.path.ends_with("com.example.app"))
        .unwrap()
        .id
        .clone();
    let trash_id = session
        .results
        .iter()
        .flat_map(|r| r.targets.iter())
        .find(|t| t.provider_id == "trash")
        .unwrap()
        .id
        .clone();

    let plan = engine.plan(
        &session,
        &[cache_id.clone(), trash_id, "bogus".into()],
        DeleteMode::Permanent,
    );
    assert_eq!(plan.targets.len(), 2);
    assert_eq!(plan.blocked.len(), 1);
    assert_eq!(plan.blocked[0].target_id, "bogus");
    assert_eq!(plan.total_bytes, 4096 + 128);
    assert_eq!(plan.directory_count, 1);
    // Dry run touched nothing.
    assert!(home
        .join("Library/Caches/com.example.app/blob.bin")
        .exists());

    let log_dir = dir.path().join("log");
    let log = OperationLog::open(&log_dir).unwrap();
    let progress_events = std::sync::atomic::AtomicUsize::new(0);
    let result = engine
        .execute(&plan, Some(&log), &|_| {
            progress_events.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        })
        .unwrap();
    assert_eq!(
        progress_events.load(std::sync::atomic::Ordering::Relaxed),
        2
    );

    assert_eq!(result.removed_targets, 2);
    assert_eq!(result.removed_bytes, 4096 + 128);
    assert!(result.failed.is_empty());
    assert!(!home.join("Library/Caches/com.example.app").exists());
    assert!(!home.join(".Trash/old.txt").exists());
    // Everything not in the plan is intact.
    assert!(home.join("Library/Caches/Homebrew/pkg.tar.gz").exists());
    assert!(home
        .join("Projects/web/node_modules/left-pad/index.js")
        .exists());
    assert!(home.join("Documents/thesis.txt").exists());
    assert!(home.join(".ssh/id_ed25519").exists());
    assert!(all_ids.len() > 2);

    let records = log.list(10).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].removed_bytes, 4096 + 128);
    assert_eq!(records[0].title, "Clean 2 categories");
}

#[test]
fn cancelled_scan_is_marked_cancelled() {
    let dir = build_sandbox();
    let engine = engine_for(&dir);
    let cancel = Arc::new(AtomicBool::new(true));
    let session = engine.scan("s3", &ScanRequest::default(), cancel, &|_| {});
    assert_eq!(session.status, prune_core::models::ScanStatus::Cancelled);
}

#[test]
fn execute_refuses_paths_that_became_protected() {
    // Simulate a target whose path is forged/outside allowed roots: plan must block it.
    let dir = build_sandbox();
    let engine = engine_for(&dir);
    let mut session = engine.scan(
        "s4",
        &ScanRequest::default(),
        Arc::new(AtomicBool::new(false)),
        &|_| {},
    );
    // Tamper with a target path to point at protected data.
    let target = session
        .results
        .iter_mut()
        .flat_map(|r| r.targets.iter_mut())
        .next()
        .unwrap();
    target.path = dir
        .path()
        .join("home/.ssh/id_ed25519")
        .to_string_lossy()
        .into_owned();
    let id = target.id.clone();
    let plan = engine.plan(&session, &[id], DeleteMode::Permanent);
    assert!(plan.targets.is_empty());
    assert_eq!(plan.blocked.len(), 1);
    assert!(dir.path().join("home/.ssh/id_ed25519").exists());
}
