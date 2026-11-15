//! Paths the user protected are never removed, nor is anything that contains one, by any route:
//! the scan labels them, a preview blocks them, and a plan made before the protection was added
//! removes nothing when it runs. When the settings holding those protections cannot be read,
//! nothing at all is removed.
//!
//! Everything lives in a sandbox under a fresh temporary directory.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use prune_core::models::{CleanupTarget, DeleteMode, RiskLevel, ScanSession};
use prune_core::platform::sandbox::SandboxPlatform;
use prune_core::providers::developer::ProjectArtifacts;
use prune_core::providers::ProviderRegistry;
use prune_core::safety::{CONTAINS_USER_PROTECTED, USER_PROTECTED};
use prune_core::scan::ScanRequest;
use prune_core::PruneEngine;

fn write(path: &Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, [b'x'; 256]).unwrap();
}

/// Three projects outside any repository, so Git has nothing to say about them.
struct Fixture {
    _dir: tempfile::TempDir,
    platform: SandboxPlatform,
    /// Protected itself.
    kept: PathBuf,
    /// Holds a protected file.
    holding: PathBuf,
    /// Nothing protected.
    free: PathBuf,
    /// The file inside `holding` that is protected.
    precious: PathBuf,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let platform = SandboxPlatform::new(dir.path());
    platform.prepare().unwrap();
    let projects = platform.home().join("Projects");
    for name in ["kept", "holding", "free"] {
        write(&projects.join(name).join("package.json"));
        write(&projects.join(name).join("node_modules/pkg/index.js"));
    }
    let precious = projects.join("holding/node_modules/pkg/LICENSE-signed.pdf");
    write(&precious);
    Fixture {
        kept: projects.join("kept/node_modules"),
        holding: projects.join("holding/node_modules"),
        free: projects.join("free/node_modules"),
        precious,
        _dir: dir,
        platform,
    }
}

fn engine(f: &Fixture) -> PruneEngine {
    PruneEngine::with_parts(
        Box::new(f.platform.clone()),
        ProviderRegistry::new(vec![Arc::new(ProjectArtifacts)]),
    )
}

fn scan(engine: &PruneEngine) -> ScanSession {
    engine.scan(
        "projects",
        &ScanRequest::default(),
        Arc::new(AtomicBool::new(false)),
        &|_| {},
    )
}

fn target<'a>(session: &'a ScanSession, path: &Path) -> &'a CleanupTarget {
    session
        .results
        .iter()
        .flat_map(|r| &r.targets)
        .find(|t| Path::new(&t.path) == path)
        .unwrap_or_else(|| panic!("{} was not reported", path.display()))
}

fn ids(session: &ScanSession) -> Vec<String> {
    session
        .results
        .iter()
        .flat_map(|r| &r.targets)
        .map(|t| t.id.clone())
        .collect()
}

#[test]
fn a_protected_folder_and_one_holding_a_protected_file_are_shown_but_never_planned() {
    let f = fixture();
    let mut engine = engine(&f);
    engine.protect(&[f.kept.clone(), f.precious.clone()]);
    let session = scan(&engine);

    let kept = target(&session, &f.kept);
    assert_eq!(kept.risk, RiskLevel::Protected);
    assert_eq!(kept.description.as_deref(), Some(USER_PROTECTED));

    let holding = target(&session, &f.holding);
    assert_eq!(holding.risk, RiskLevel::Protected);
    assert_eq!(
        holding.description.as_deref(),
        Some(CONTAINS_USER_PROTECTED)
    );

    assert_eq!(target(&session, &f.free).risk, RiskLevel::Low);

    let plan = engine.plan(&session, &ids(&session), DeleteMode::Permanent);
    let planned: Vec<&Path> = plan.targets.iter().map(|t| Path::new(&t.path)).collect();
    assert_eq!(planned, vec![f.free.as_path()]);

    let result = engine.execute(&plan, None, &|_| {}).unwrap();
    assert_eq!(result.removed_targets, 1);
    assert!(f.kept.exists());
    assert!(f.precious.exists());
    assert!(!f.free.exists());
}

#[test]
fn a_plan_made_before_the_protection_removes_nothing_it_now_covers() {
    let f = fixture();
    let before = engine(&f);
    let session = scan(&before);
    let plan = before.plan(&session, &ids(&session), DeleteMode::Permanent);
    assert_eq!(plan.targets.len(), 3, "nothing is protected yet");

    // The app rebuilds its engine when settings change; the old plan runs on the new one.
    let mut after = engine(&f);
    after.protect(&[f.kept.clone(), f.precious.clone()]);
    let result = after.execute(&plan, None, &|_| {}).unwrap();

    assert_eq!(result.removed_targets, 1);
    let mut refused: Vec<&str> = result.failed.iter().map(|t| t.error.as_str()).collect();
    refused.sort();
    assert!(refused[0].contains(CONTAINS_USER_PROTECTED), "{refused:?}");
    assert!(refused[1].contains(USER_PROTECTED), "{refused:?}");
    assert!(f.kept.exists());
    assert!(f.precious.exists());
}

#[test]
fn a_protected_path_that_is_missing_is_protected_again_when_it_returns() {
    let f = fixture();
    let elsewhere = f.platform.home().join("Projects/later/node_modules");
    let mut engine = engine(&f);
    // Not there yet: an unplugged drive, a folder about to be restored.
    engine.protect(std::slice::from_ref(&elsewhere));

    write(&f.platform.home().join("Projects/later/package.json"));
    write(&elsewhere.join("pkg/index.js"));
    let session = scan(&engine);
    assert_eq!(target(&session, &elsewhere).risk, RiskLevel::Protected);
    let plan = engine.plan(&session, &ids(&session), DeleteMode::Permanent);
    assert!(plan.targets.iter().all(|t| Path::new(&t.path) != elsewhere));
}

#[test]
fn while_protections_are_unknown_nothing_is_removed() {
    let f = fixture();
    let before = engine(&f);
    let early_plan = {
        let session = scan(&before);
        before.plan(&session, &ids(&session), DeleteMode::Permanent)
    };

    let mut engine = engine(&f);
    engine.refuse_all_removal("the settings file could not be read");
    let session = scan(&engine);
    for t in session.results.iter().flat_map(|r| &r.targets) {
        assert_eq!(t.risk, RiskLevel::Protected, "{}", t.path);
    }
    let plan = engine.plan(&session, &ids(&session), DeleteMode::Permanent);
    assert!(plan.targets.is_empty());

    let result = engine.execute(&early_plan, None, &|_| {}).unwrap();
    assert_eq!(result.removed_targets, 0);
    assert_eq!(result.failed.len(), 3);
    assert!(result.failed[0]
        .error
        .contains("the settings file could not be read"));
    assert!(f.kept.exists() && f.holding.exists() && f.free.exists());
}
