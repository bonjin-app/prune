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
