//! Project artifact fixtures exercise detection and the existing scan/preview/cleanup pipeline.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use prune_core::models::{DeleteMode, RiskLevel};
use prune_core::platform::sandbox::SandboxPlatform;
use prune_core::platform::PlatformService;
use prune_core::providers::developer::ProjectArtifacts;
use prune_core::providers::{CleanupProvider, ProviderRegistry, ScanContext, ScanOutput};
use prune_core::safety::SafetyPolicy;
use prune_core::scan::ScanRequest;
use prune_core::PruneEngine;

fn write(path: &Path, contents: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

fn engine(platform: SandboxPlatform) -> PruneEngine {
    PruneEngine::with_parts(
        Box::new(platform),
        ProviderRegistry::new(vec![Arc::new(ProjectArtifacts)]),
    )
}

fn scan_project_roots(platform: &SandboxPlatform, roots: Vec<PathBuf>) -> ScanOutput {
    let mut known = platform.known_paths();
    known.project_roots = roots;
    let policy = SafetyPolicy::for_platform(platform, &known);
    ProjectArtifacts.scan(&ScanContext {
        known: &known,
        platform,
        policy: &policy,
        cancel: &AtomicBool::new(false),
        progress: &|_, _, _| {},
    })
}

fn artifact_like_sources(root: &Path) -> Vec<PathBuf> {
    [
        "package.json",
        "dist/source.jslib",
        "node_modules/source.js",
        "build/source.bundle",
        "__pycache__/source.pyc",
        ".pytest_cache/source",
    ]
    .into_iter()
    .map(|relative| {
        let path = root.join(relative);
        write(&path, b"keep");
        path
    })
    .collect()
}

#[test]
fn unity_source_and_nested_source_roots_never_offer_artifacts() {
    let dir = tempfile::tempdir().unwrap();
    let platform = SandboxPlatform::new(dir.path());
    platform.prepare().unwrap();
    let unity = platform.home().join("Projects/unity");
    let roots = [
        "Assets",
        "Assets/Plugins/vendor",
        "Packages",
        "ProjectSettings",
    ]
    .map(|relative| unity.join(relative));
    let preserved: Vec<_> = roots
        .iter()
        .flat_map(|root| artifact_like_sources(root))
        .collect();
    let library = unity.join("Library");
    write(&library.join("ArtifactDB"), b"generated");

    // Even a nested project's own Unity markers must not authorize deleting source assets.
    let nested_unity = unity.join("Assets/EmbeddedProject");
    write(&nested_unity.join("Assets/source.unity"), b"keep");
    write(
        &nested_unity.join("ProjectSettings/ProjectVersion.txt"),
        b"keep",
    );
    write(&nested_unity.join("Library/source.bin"), b"keep");

    for root in roots {
        let output = scan_project_roots(&platform, vec![root.clone()]);
        assert!(
            output.issues.is_empty(),
            "{}: {:?}",
            root.display(),
            output.issues
        );
        assert!(
            output.targets.is_empty(),
            "{}: {:?}",
            root.display(),
            output.targets
        );
    }
    let output = scan_project_roots(&platform, vec![unity]);
    assert!(output.issues.is_empty());
    assert_eq!(output.targets.len(), 1);
    assert_eq!(Path::new(&output.targets[0].path), library);
    assert_eq!(output.targets[0].risk, RiskLevel::Low);
    for path in preserved {
        assert_eq!(std::fs::read(path).unwrap(), b"keep");
    }
    assert_eq!(
        std::fs::read(nested_unity.join("Library/source.bin")).unwrap(),
        b"keep"
    );
}

#[test]
fn arbitrary_source_named_roots_without_unity_markers_still_offer_artifacts() {
    let dir = tempfile::tempdir().unwrap();
    let platform = SandboxPlatform::new(dir.path());
    platform.prepare().unwrap();
    for name in ["Assets", "Packages", "ProjectSettings"] {
        let root = platform
            .home()
            .join("Projects")
            .join(format!("{name}-only"))
            .join(name);
        let preserved = artifact_like_sources(&root);
        let output = scan_project_roots(&platform, vec![root.clone()]);
        assert!(output.issues.is_empty());
        let mut actual: Vec<_> = output
            .targets
            .iter()
            .map(|target| PathBuf::from(&target.path))
            .collect();
        actual.sort();
        let mut expected = [
            "dist",
            "node_modules",
            "build",
            "__pycache__",
            ".pytest_cache",
        ]
        .map(|name| root.join(name));
        expected.sort();
        assert_eq!(
            actual,
            expected,
            "{} is not a Unity project",
            root.display()
        );
        for path in preserved {
            assert_eq!(std::fs::read(path).unwrap(), b"keep");
        }
    }
}

#[cfg(unix)]
#[test]
fn unity_source_root_symlink_aliases_do_not_bypass_protection() {
    let dir = tempfile::tempdir().unwrap();
    let platform = SandboxPlatform::new(dir.path());
    platform.prepare().unwrap();
    let unity = platform.home().join("Projects/unity");
    let sources = unity.join("Assets/Plugins");
    let preserved = artifact_like_sources(&sources);
    std::fs::create_dir_all(unity.join("ProjectSettings")).unwrap();
    let alias = platform.home().join("source-alias");
    std::os::unix::fs::symlink(&sources, &alias).unwrap();
    let output = scan_project_roots(&platform, vec![alias]);
    assert!(output.issues.is_empty());
    assert!(output.targets.is_empty(), "{:?}", output.targets);
    for path in preserved {
        assert_eq!(std::fs::read(path).unwrap(), b"keep");
    }
}

#[cfg(unix)]
#[test]
fn unity_source_symlink_root_does_not_scan_its_external_target() {
    let dir = tempfile::tempdir().unwrap();
    let platform = SandboxPlatform::new(dir.path());
    platform.prepare().unwrap();
    let unity = platform.home().join("Projects/unity");
    std::fs::create_dir_all(unity.join("Assets")).unwrap();
    std::fs::create_dir(unity.join("ProjectSettings")).unwrap();
    let external = platform.home().join("package-source");
    let preserved = artifact_like_sources(&external);
    let packages = unity.join("Packages");
    std::os::unix::fs::symlink(external, &packages).unwrap();

    let output = scan_project_roots(&platform, vec![packages]);
    assert!(output.issues.is_empty());
    assert!(output.targets.is_empty(), "{:?}", output.targets);
    for path in preserved {
        assert_eq!(std::fs::read(path).unwrap(), b"keep");
    }
}

#[test]
fn unity_library_preview_and_cleanup_preserve_source_and_unrelated_libraries() {
    let dir = tempfile::tempdir().unwrap();
    let platform = SandboxPlatform::new(dir.path());
    platform.prepare().unwrap();
    let projects = platform.home().join("Projects");
    let unity = projects.join("games/unity");
    let library = unity.join("Library");
    write(&library.join("ArtifactDB"), b"generated");
    write(&library.join("__pycache__/nested.pyc"), b"nested");

    // Even cache-like paths inside Unity source/config directories must be left alone.
    let preserved = [
        unity.join("Assets/Scenes/Main.unity"),
        unity.join("Assets/__pycache__/keep.pyc"),
        unity.join("Packages/manifest.json"),
        unity.join("Packages/package.json"),
        unity.join("Packages/node_modules/keep.js"),
        unity.join("ProjectSettings/ProjectVersion.txt"),
        unity.join("ProjectSettings/.pytest_cache/keep"),
        projects.join("notes/Library/__pycache__/keep.pyc"),
        projects.join("assets-only/Assets/source"),
        projects.join("assets-only/Library/keep"),
        projects.join("settings-only/ProjectSettings/source"),
        projects.join("settings-only/Library/keep"),
        // Markers in an ancestor do not authorize a nested arbitrary Library.
        unity.join("tools/Library/keep"),
    ];
    for path in &preserved {
        write(path, b"keep");
    }

    let engine = engine(platform);
    let session = engine.scan(
        "unity",
        &ScanRequest::default(),
        Arc::new(AtomicBool::new(false)),
        &|_| {},
    );
    assert!(session
        .results
        .iter()
        .all(|result| result.issues.is_empty()));
    let targets: Vec<_> = session.results.iter().flat_map(|r| &r.targets).collect();
    assert_eq!(
        targets.len(),
        1,
        "only the evidenced Unity Library is a cache"
    );
    let target = targets[0];
    assert_eq!(
        Path::new(&target.path).canonicalize().unwrap(),
        library.canonicalize().unwrap()
    );
    assert_eq!(target.risk, RiskLevel::Low);
    assert_eq!(target.size_bytes, 15);
    assert_eq!(target.file_count, 2);
    assert_eq!(target.group.as_ref().unwrap().label, "unity");

    let plan = engine.plan(
        &session,
        std::slice::from_ref(&target.id),
        DeleteMode::Permanent,
    );
    assert!(plan.blocked.is_empty());
    assert_eq!(plan.targets.len(), 1);
    assert_eq!(plan.total_bytes, 15);
    assert_eq!(plan.directory_count, 1);
    assert!(
        library.join("ArtifactDB").exists(),
        "preview must not delete"
    );
    for path in &preserved {
        assert_eq!(std::fs::read(path).unwrap(), b"keep");
    }

    let result = engine.execute(&plan, None, &|_| {}).unwrap();
    assert_eq!(result.removed_targets, 1);
    assert_eq!(result.removed_bytes, 15);
    assert!(result.failed.is_empty());
    assert!(!library.exists());
    for path in &preserved {
        assert_eq!(std::fs::read(path).unwrap(), b"keep");
    }
}

#[cfg(unix)]
#[test]
fn unity_library_symlinks_are_not_followed_or_reported() {
    let dir = tempfile::tempdir().unwrap();
    let platform = SandboxPlatform::new(dir.path());
    platform.prepare().unwrap();
    let unity = platform.home().join("Projects/unity");
    std::fs::create_dir_all(unity.join("Assets")).unwrap();
    std::fs::create_dir(unity.join("ProjectSettings")).unwrap();
    let external = dir.path().join("external");
    write(&external.join("__pycache__/keep.pyc"), b"keep");
    std::os::unix::fs::symlink(&external, unity.join("Library")).unwrap();

    let session = engine(platform).scan(
        "unity-symlink",
        &ScanRequest::default(),
        Arc::new(AtomicBool::new(false)),
        &|_| {},
    );
    assert!(session.results.iter().all(|r| r.targets.is_empty()));
    assert_eq!(
        std::fs::read(external.join("__pycache__/keep.pyc")).unwrap(),
        b"keep"
    );
}

/// Runs `git` in `dir` with nothing from the user's identity or signing setup involved:
/// `init` and `add` only, never a commit.
fn git(platform: &SandboxPlatform, dir: &Path, args: &[&str]) {
    let git = platform.git().expect("these tests need git installed");
    let status = std::process::Command::new(git)
        .args(args)
        .current_dir(dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

/// A repository with a JavaScript action that commits `dist/`, beside a web app whose `dist/`
/// and `node_modules/` are ignored output, and a `build/` that is a clone of something else.
struct Repo {
    _dir: tempfile::TempDir,
    platform: SandboxPlatform,
    committed_dist: PathBuf,
    ignored_dist: PathBuf,
    node_modules: PathBuf,
    nested_repo: PathBuf,
}

fn repo() -> Repo {
    let dir = tempfile::tempdir().unwrap();
    let platform = SandboxPlatform::new(dir.path());
    platform.prepare().unwrap();
    let root = platform.home().join("Projects/monorepo");
    std::fs::create_dir_all(&root).unwrap();
    git(&platform, &root, &["init", "-q"]);

    write(&root.join("action/package.json"), b"{}");
    write(&root.join("action/dist/index.js"), &[b'x'; 2048]);
    git(
        &platform,
        &root,
        &["add", "action/package.json", "action/dist/index.js"],
    );

    write(&root.join("web/package.json"), b"{}");
    write(&root.join("web/dist/bundle.js"), &[b'x'; 4096]);
    write(
        &root.join("web/node_modules/left-pad/index.js"),
        &[b'x'; 1024],
    );
    write(&root.join("web/.gitignore"), b"dist\nnode_modules\n");
    git(
        &platform,
        &root,
        &["add", "web/package.json", "web/.gitignore"],
    );

    write(&root.join("native/CMakeLists.txt"), b"");
    write(&root.join("native/build/README"), b"x");
    git(&platform, &root.join("native/build"), &["init", "-q"]);

    Repo {
        committed_dist: root.join("action/dist"),
        ignored_dist: root.join("web/dist"),
        node_modules: root.join("web/node_modules"),
        nested_repo: root.join("native/build"),
        _dir: dir,
        platform,
    }
}

#[test]
fn build_folders_git_tracks_are_kept_and_never_planned() {
    let r = repo();
    let engine = engine(r.platform.clone());
    let session = engine.scan(
        "projects",
        &ScanRequest::default(),
        Arc::new(AtomicBool::new(false)),
        &|_| {},
    );
    let targets: Vec<_> = session
        .results
        .iter()
        .flat_map(|res| &res.targets)
        .collect();
    let find = |path: &Path| {
        targets
            .iter()
            .find(|t| Path::new(&t.path) == path)
            .unwrap_or_else(|| panic!("{} was not reported", path.display()))
    };

    let committed = find(&r.committed_dist);
    assert_eq!(committed.risk, RiskLevel::Protected);
    assert_eq!(
        committed.description.as_deref(),
        Some("dist (committed to Git)")
    );
    assert_eq!(committed.size_bytes, 2048, "still shown with its size");

    let nested = find(&r.nested_repo);
    assert_eq!(nested.risk, RiskLevel::Protected);
    assert_eq!(
        nested.description.as_deref(),
        Some("build (its own Git repository)")
    );

    for ignored in [&r.ignored_dist, &r.node_modules] {
        let t = find(ignored);
        assert_eq!(
            t.risk,
            RiskLevel::Low,
            "{} is ignored output",
            ignored.display()
        );
    }
    assert!(
        session.results.iter().all(|res| res.issues.is_empty()),
        "{:?}",
        session
            .results
            .iter()
            .map(|res| &res.issues)
            .collect::<Vec<_>>()
    );

    // Asked for by id anyway, the kept ones are blocked rather than planned.
    let ids: Vec<String> = targets.iter().map(|t| t.id.clone()).collect();
    let plan = engine.plan(&session, &ids, DeleteMode::Trash);
    let planned: Vec<&str> = plan.targets.iter().map(|t| t.path.as_str()).collect();
    assert_eq!(plan.targets.len(), 2, "{planned:?}");
    for kept in [&r.committed_dist, &r.nested_repo] {
        assert!(
            !planned.contains(&kept.to_str().unwrap()),
            "{} was planned",
            kept.display()
        );
        assert!(plan
            .blocked
            .iter()
            .any(|b| Path::new(&b.path) == kept.as_path()));
    }
}

#[test]
fn a_build_folder_committed_after_the_scan_is_kept_at_preview_and_at_removal() {
    let r = repo();
    let engine = engine(r.platform.clone());
    let session = engine.scan(
        "projects",
        &ScanRequest::default(),
        Arc::new(AtomicBool::new(false)),
        &|_| {},
    );
    let ignored = session
        .results
        .iter()
        .flat_map(|res| &res.targets)
        .find(|t| Path::new(&t.path) == r.ignored_dist)
        .expect("the ignored dist is offered")
        .clone();
    assert_eq!(ignored.risk, RiskLevel::Low);

    // Planned while it was still ignored output.
    let early_plan = engine.plan(
        &session,
        std::slice::from_ref(&ignored.id),
        DeleteMode::Trash,
    );
    assert_eq!(early_plan.targets.len(), 1);

    // Then someone commits it.
    let root = r
        .ignored_dist
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    git(&r.platform, &root, &["add", "-f", "web/dist/bundle.js"]);

    // A preview made now refuses it, with the reason.
    let plan = engine.plan(
        &session,
        std::slice::from_ref(&ignored.id),
        DeleteMode::Trash,
    );
    assert!(plan.targets.is_empty());
    assert_eq!(plan.blocked.len(), 1);
    assert_eq!(plan.blocked[0].reason, "Git now tracks files in it");

    // And the plan made before the commit removes nothing when it runs.
    let result = engine.execute(&early_plan, None, &|_| {}).unwrap();
    assert_eq!(result.removed_targets, 0);
    assert_eq!(result.failed.len(), 1);
    assert_eq!(result.failed[0].error, "kept: Git now tracks files in it");
    assert!(
        r.ignored_dist.join("bundle.js").exists(),
        "the folder is still there"
    );
}

#[test]
fn ignored_output_is_still_removed_after_the_second_check() {
    let r = repo();
    let engine = engine(r.platform.clone());
    let session = engine.scan(
        "projects",
        &ScanRequest::default(),
        Arc::new(AtomicBool::new(false)),
        &|_| {},
    );
    let node_modules = session
        .results
        .iter()
        .flat_map(|res| &res.targets)
        .find(|t| Path::new(&t.path) == r.node_modules)
        .unwrap()
        .clone();
    let plan = engine.plan(&session, &[node_modules.id], DeleteMode::Permanent);
    let result = engine.execute(&plan, None, &|_| {}).unwrap();
    assert_eq!(result.removed_targets, 1, "{:?}", result.failed);
    assert!(!r.node_modules.exists());
}
