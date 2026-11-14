//! The temporary folder: what a running program is using is shown but never offered, and what
//! belongs to macOS is left alone.
//!
//! Every file here lives in a sandbox under a fresh temporary directory. The "running program"
//! is this test process, holding a file open the way a browser holds its singleton socket.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use prune_core::models::{CleanupTarget, DeleteMode, RiskLevel, ScanSession};
use prune_core::platform::sandbox::SandboxPlatform;
use prune_core::platform::{InUse, KnownPaths, PlatformService, ProtectedPaths};
use prune_core::providers::{system, ProviderRegistry};
use prune_core::scan::ScanRequest;
use prune_core::PruneEngine;

const TWO_DAYS: Duration = Duration::from_secs(2 * 86_400);

fn engine(platform: impl PlatformService + 'static) -> PruneEngine {
    PruneEngine::with_parts(
        Box::new(platform),
        ProviderRegistry::new(vec![Arc::new(system::TEMP_FILES)]),
    )
}

fn scan(engine: &PruneEngine) -> ScanSession {
    engine.scan(
        "temp",
        &ScanRequest::default(),
        Arc::new(AtomicBool::new(false)),
        &|_| {},
    )
}

fn target<'a>(session: &'a ScanSession, path: &Path) -> Option<&'a CleanupTarget> {
    session
        .results
        .iter()
        .flat_map(|r| &r.targets)
        .find(|t| Path::new(&t.path) == path)
}

fn write(path: &Path, size: usize) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, vec![b'x'; size]).unwrap();
}

/// Moves a file's or directory's modification time two days into the past.
fn age(path: &Path) {
    let when = SystemTime::now() - TWO_DAYS;
    #[cfg(unix)]
    let file = File::open(path).unwrap();
    #[cfg(windows)]
    let file = {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_WRITE_ATTRIBUTES: u32 = 0x0100;
        // A directory can only be opened as a handle with backup semantics.
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        std::fs::OpenOptions::new()
            .access_mode(FILE_WRITE_ATTRIBUTES)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(path)
            .unwrap()
    };
    file.set_times(std::fs::FileTimes::new().set_modified(when))
        .unwrap();
}

/// An old folder nothing is using, an old file this process holds open, and an old macOS
/// service folder.
struct Fixture {
    _dir: tempfile::TempDir,
    platform: SandboxPlatform,
    idle: PathBuf,
    held_file: PathBuf,
    apple: PathBuf,
    /// Keeps `held_file` open for as long as the fixture lives.
    _handle: File,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let platform = SandboxPlatform::new(dir.path());
    platform.prepare().unwrap();
    let tmp = platform.temp();

    let idle = tmp.join("old-download-123");
    write(&idle.join("part.bin"), 400);
    age(&idle);

    let held_file = tmp.join("app.501.lock");
    let handle = File::create(&held_file).unwrap();
    std::io::Write::write_all(&mut &handle, &[b'x'; 64]).unwrap();
    handle
        .set_times(std::fs::FileTimes::new().set_modified(SystemTime::now() - TWO_DAYS))
        .unwrap();

    let apple = tmp.join("com.apple.example");
    write(&apple.join("state.db"), 300);
    age(&apple);

    Fixture {
        _dir: dir,
        platform,
        idle,
        held_file,
        apple,
        _handle: handle,
    }
}

#[test]
fn an_old_folder_nothing_is_using_is_still_offered() {
    let f = fixture();
    let session = scan(&engine(f.platform.clone()));
    let idle = target(&session, &f.idle).expect("the idle folder is found");
    assert_eq!(idle.risk, RiskLevel::Low);
    assert_eq!(idle.description, None);
}

#[test]
fn a_file_a_running_program_holds_open_is_protected_not_offered() {
    let f = fixture();
    let engine = engine(f.platform.clone());
    let session = scan(&engine);
    let held = target(&session, &f.held_file).expect("an in-use item is still shown");
    assert_eq!(held.risk, RiskLevel::Protected);
    assert_eq!(held.size_bytes, 64, "shown with its size");
    assert!(
        held.description
            .as_deref()
            .is_some_and(|d| d.starts_with("In use by a running program")),
        "{:?}",
        held.description
    );

    // Even asked for by id, it is blocked rather than planned.
    let plan = engine.plan(&session, std::slice::from_ref(&held.id), DeleteMode::Trash);
    assert!(plan.targets.is_empty());
    assert_eq!(plan.blocked.len(), 1);
}

/// A folder with something open deep inside it. macOS reports every open file, so the folder
/// that contains one is known to be in use; Windows refuses to move such a folder by itself,
/// and is not asked.
#[cfg(target_os = "macos")]
#[test]
fn a_folder_with_an_open_file_inside_is_protected() {
    let f = fixture();
    let folder = f.platform.temp().join("com.example.app.Xy12");
    let inside = folder.join("session/SingletonLock");
    write(&inside, 32);
    let _open = File::open(&inside).unwrap();
    age(&folder);

    let session = scan(&engine(f.platform.clone()));
    let busy = target(&session, &folder).expect("the folder is shown");
    assert_eq!(busy.risk, RiskLevel::Protected);
    let description = busy.description.as_deref().unwrap_or_default();
    assert!(
        description.starts_with("In use by a running program ("),
        "names the program: {description}"
    );
    // The folder next to it is unaffected.
    assert_eq!(target(&session, &f.idle).unwrap().risk, RiskLevel::Low);
}

#[test]
fn com_apple_folders_are_not_offered() {
    let f = fixture();
    let session = scan(&engine(f.platform.clone()));
    assert!(target(&session, &f.apple).is_none());
    assert!(target(&session, &f.idle).is_some());
}

/// A platform that cannot tell what is in use, wrapped around the sandbox.
struct Blind(SandboxPlatform);

impl PlatformService for Blind {
    fn platform(&self) -> prune_core::models::Platform {
        self.0.platform()
    }
    fn known_paths(&self) -> KnownPaths {
        self.0.known_paths()
    }
    fn protected_paths(&self, known: &KnownPaths) -> ProtectedPaths {
        self.0.protected_paths(known)
    }
    fn in_use(&self, _candidates: &[PathBuf]) -> InUse {
        InUse::Unknown("lsof is missing".into())
    }
}

#[test]
fn when_use_cannot_be_checked_the_scan_says_so() {
    let f = fixture();
    let session = scan(&engine(Blind(f.platform.clone())));
    let result = &session.results[0];
    assert!(
        result
            .issues
            .iter()
            .any(|i| i.message.contains("Could not check") && i.message.contains("lsof is missing")),
        "{:?}",
        result.issues
    );
    // Failing open: nothing is claimed to be in use, so the old folder is offered as before.
    assert_eq!(target(&session, &f.idle).unwrap().risk, RiskLevel::Low);
}
