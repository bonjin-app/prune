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

/// Does this path end with `suffix`, whichever separators either happens to use?
///
/// Paths are compared as text here, and on Windows `join("Projects/web")` keeps the forward
/// slash inside the component while adding backslashes around it — so a literal match misses on
/// both spellings.
fn ends_with_path(path: &str, suffix: &str) -> bool {
    path.replace('\\', "/")
        .ends_with(&suffix.replace('\\', "/"))
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
        paths
            .iter()
            .any(|(p, path)| p == provider && ends_with_path(path, suffix))
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
        .find(|t| ends_with_path(&t.path, "Projects/web/node_modules"))
        .unwrap();
    assert_eq!(node_modules.size_bytes, 300);
    assert_eq!(node_modules.file_count, 1);
    assert_eq!(node_modules.risk, RiskLevel::Low);
    assert!(session.total_bytes >= 4096 + 2048 + 512 + 1024 + 300 + 700 + 128);
}

#[test]
fn no_provider_reaches_outside_the_directories_it_was_given() {
    // The property every provider has to hold, whatever platform it resolves its paths for:
    // a target lives strictly inside one of the directories `KnownPaths` named, and is never
    // one of those directories itself. A provider that claimed `~/Library/Caches` or
    // `%LOCALAPPDATA%` whole would offer to delete every application's data at once.
    //
    // This runs on macOS and on Windows in CI, so both halves of every `cfg!` branch are
    // covered without either being written out by hand.
    let dir = build_sandbox();
    let home = dir.path().join("home");
    let engine = engine_for(&dir);

    // One provider at a time. A whole-directory scan resolves overlaps between providers, and
    // a target that swallows a higher-priority one is exactly what that resolution removes —
    // so the offence would be tidied away before this could see it.
    let mut sessions = Vec::new();
    for info in engine.providers() {
        let id = info.id.clone();
        sessions.push(engine.scan(
            &format!("invariants-{id}"),
            &ScanRequest {
                provider_ids: Some(vec![id]),
            },
            Arc::new(AtomicBool::new(false)),
            &|_| {},
        ));
    }

    // Compared in canonical form. The scan normalises what it reports, so on macOS a plain
    // comparison pits `/var/folders/…` against `/private/var/folders/…` and quietly matches
    // nothing — which is how the first version of this test passed while a provider really was
    // claiming a whole cache directory.
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let known = engine.known_paths();
    let roots: Vec<PathBuf> = [
        known.user_cache.clone(),
        known.user_logs.clone(),
        known.app_support.clone(),
        known.local_app_data.clone(),
        known.trash.clone(),
        known.downloads.clone(),
        Some(known.temp.clone()),
        Some(known.home.clone()),
    ]
    .into_iter()
    .flatten()
    .map(|p| canon(&p))
    .collect();
    let home_c = canon(&home);
    let temp_c = canon(&known.temp);

    let mut reported = 0usize;
    for result in sessions.iter().flat_map(|s| s.results.iter()) {
        for target in &result.targets {
            reported += 1;
            let path = canon(&PathBuf::from(&target.path));

            assert!(
                path.starts_with(&home_c) || path.starts_with(&temp_c),
                "{} reported {} , which is outside the sandbox",
                result.provider_id,
                target.path
            );
            assert!(
                !roots.iter().any(|r| &path == r),
                "{} reported {} , which is a whole user directory rather than something inside one",
                result.provider_id,
                target.path
            );
        }
    }
    assert!(
        reported > 0,
        "the fixture should produce something to check"
    );
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
        .find(|t| ends_with_path(&t.path, app_cache_name()))
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
        .find(|t| ends_with_path(&t.path, app_cache_name()))
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
        .find(|t| ends_with_path(&t.path, app_cache_name()))
        .unwrap()
        .id
        .clone();
    let plan = engine.plan(&session, &[id], DeleteMode::Permanent);
    let log = OperationLog::open(&dir.path().join("goodlog"));

    let result = engine.execute(&plan, Some(&log), &|_| {}).unwrap();

    assert!(result.log_error.is_none());
    assert_eq!(log.list(10).unwrap().len(), 1);
}
