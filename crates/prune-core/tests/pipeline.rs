//! End-to-end test of the safety pipeline against a sandboxed fake home directory.
//! Never touches the real filesystem outside a temp dir.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use prune_core::models::{DeleteMode, RiskLevel};
use prune_core::ops::OperationLog;
use prune_core::platform::sandbox::SandboxPlatform;
use prune_core::providers::ProviderRegistry;
use prune_core::scan::ScanRequest;
use prune_core::PruneEngine;

fn write(path: &Path, size: usize) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, vec![b'x'; size]).unwrap();
}

/// The cache the generic provider collects here, and the name its target path ends with.
///
/// macOS has one `Caches` directory and every child of it is a target. Windows has no such
/// directory, so the provider names specific well-known locations and each location is itself
/// the target. The pipeline under test is the same either way; only where the fixture has to
/// put a cache differs.
fn app_cache(home: &Path) -> PathBuf {
    if cfg!(windows) {
        home.join("Library/Application Support/D3DSCache")
    } else {
        home.join("Library/Caches/com.example.app")
    }
}

/// The npm cache this platform's provider collects: `~/.npm/_cacache` on macOS,
/// `%LOCALAPPDATA%\npm-cache` on Windows.
fn npm_cache(home: &Path) -> PathBuf {
    if cfg!(windows) {
        home.join("Library/Application Support/npm-cache")
    } else {
        home.join(".npm/_cacache")
    }
}

/// The name that npm cache's target path ends with.
fn npm_cache_name() -> &'static str {
    if cfg!(windows) {
        "npm-cache"
    } else {
        "_cacache"
    }
}

/// The name that cache's target path ends with.
fn app_cache_name() -> &'static str {
    if cfg!(windows) {
        "D3DSCache"
    } else {
        "com.example.app"
    }
}

fn build_sandbox() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    // caches (safe)
    write(&home.join("Library/Caches/com.example.app/blob.bin"), 4096);
    // Same size, so the totals asserted below hold on either platform.
    if cfg!(windows) {
        write(&app_cache(&home).join("shader.bin"), 4096);
    }
    write(&home.join("Library/Caches/Homebrew/pkg.tar.gz"), 2048);
    // logs
    write(&home.join("Library/Logs/app.log"), 512);
    // npm cache (specialised provider)
    write(&npm_cache(&home).join("index/x"), 1024);
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
        Box::new(SandboxPlatform::new(dir.path())),
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
    // Separators are normalised: on Windows `join("Library/Caches")` keeps the forward slash
    // inside the component and adds backslashes around it, so a plain string match on either
    // form misses.
    let has = |provider: &str, suffix: &str| {
        paths.iter().any(|(p, path)| {
            p == provider
                && path
                    .replace('\\', "/")
                    .ends_with(&suffix.replace('\\', "/"))
        })
    };

    assert!(has("user_cache", app_cache_name()));
    // Homebrew does not exist on Windows, so neither does the overlap it is here to prove: the
    // specialised provider must win over the generic one for the same directory.
    if cfg!(unix) {
        assert!(has("homebrew_cache", "Library/Caches/Homebrew"));
        assert!(!has("user_cache", "Library/Caches/Homebrew"));
    }
    assert!(has("user_logs", "Library/Logs/app.log"));
    assert!(has("npm_cache", npm_cache_name()));
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
        .find(|t| t.path.ends_with(app_cache_name()))
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
    assert!(app_cache(&home).exists());

    let log_dir = dir.path().join("log");
    let log = OperationLog::open(&log_dir);
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
    assert!(!app_cache(&home).exists());
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

#[test]
fn a_cleanup_still_counts_when_the_log_cannot_be_written() {
    let dir = build_sandbox();
    let engine = engine_for(&dir);
    let home = dir.path().join("home");
    let session = engine.scan(
        "s5",
        &ScanRequest::default(),
        Arc::new(AtomicBool::new(false)),
        &|_| {},
    );
    let cache = session
        .results
        .iter()
        .flat_map(|r| r.targets.iter())
        .find(|t| t.path.ends_with(app_cache_name()))
        .unwrap()
        .clone();
    let plan = engine.plan(
        &session,
        std::slice::from_ref(&cache.id),
        DeleteMode::Permanent,
    );

    // A log that cannot be opened: the path exists as a file, so creating it as a directory
    // fails. This is the shape of a read-only or full disk.
    let blocked = dir.path().join("blocked");
    std::fs::write(&blocked, b"not a directory").unwrap();
    let log = OperationLog::open(&blocked);

    let result = engine
        .execute(&plan, Some(&log), &|_| {})
        .expect("a log failure must not fail the cleanup");
    assert!(
        result.log_error.is_some(),
        "the user should be told the history was not recorded"
    );

    // What matters: the files really are gone and the caller was told so.
    assert_eq!(result.removed_targets, 1);
    assert_eq!(result.removed_bytes, 4096);
    assert!(!app_cache(&home).exists());
}

#[test]
fn a_successful_cleanup_reports_no_log_problem() {
    let dir = build_sandbox();
    let engine = engine_for(&dir);
    let session = engine.scan(
        "s6",
        &ScanRequest::default(),
        Arc::new(AtomicBool::new(false)),
        &|_| {},
    );
    let id = session
        .results
        .iter()
        .flat_map(|r| r.targets.iter())
        .find(|t| t.path.ends_with(app_cache_name()))
        .unwrap()
        .id
        .clone();
    let plan = engine.plan(&session, &[id], DeleteMode::Permanent);
    let log = OperationLog::open(&dir.path().join("goodlog"));

    let result = engine.execute(&plan, Some(&log), &|_| {}).unwrap();

    assert!(result.log_error.is_none());
    assert_eq!(log.list(10).unwrap().len(), 1);
}
