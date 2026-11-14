//! What AI tools keep on disk is found, and what cannot be downloaded again is never removable.
//!
//! Every layout here is built in a temporary home directory, the way each tool lays it out by
//! default. None of these tests reads the real home directory.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use prune_core::models::{Category, CleanupTarget, DeleteMode, RiskLevel, ScanSession};
use prune_core::platform::sandbox::SandboxPlatform;
use prune_core::providers::{default_providers, ProviderRegistry};
use prune_core::scan::ScanRequest;
use prune_core::PruneEngine;

fn write(path: &Path, size: usize) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, vec![b'x'; size]).unwrap();
}

/// The engine with only the AI providers, so other providers' findings do not get in the way.
fn engine(platform: SandboxPlatform) -> PruneEngine {
    let ai = default_providers()
        .into_iter()
        .filter(|p| p.category() == Category::AiData)
        .collect();
    PruneEngine::with_parts(Box::new(platform), ProviderRegistry::new(ai))
}

fn scan(engine: &PruneEngine) -> ScanSession {
    engine.scan(
        "ai",
        &ScanRequest::default(),
        Arc::new(AtomicBool::new(false)),
        &|_| {},
    )
}

fn targets(session: &ScanSession) -> Vec<&CleanupTarget> {
    session.results.iter().flat_map(|r| &r.targets).collect()
}

/// Provider id → the paths it reported, relative to `home`, with `/` separators.
fn by_provider(session: &ScanSession, home: &Path) -> BTreeMap<String, Vec<String>> {
    let home = home.canonicalize().unwrap();
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for t in targets(session) {
        let path = PathBuf::from(&t.path);
        let path = path.canonicalize().unwrap_or(path);
        let relative = path
            .strip_prefix(&home)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        map.entry(t.provider_id.clone()).or_default().push(relative);
    }
    for paths in map.values_mut() {
        paths.sort();
    }
    map
}

/// Moves a directory's modification time into the past, as if nothing had touched it since.
fn age(dir: &Path, by: Duration) {
    let when = SystemTime::now() - by;
    #[cfg(unix)]
    let file = std::fs::File::open(dir).unwrap();
    #[cfg(windows)]
    let file = {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_WRITE_ATTRIBUTES: u32 = 0x0100;
        // A directory can only be opened as a handle with backup semantics.
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        std::fs::OpenOptions::new()
            .access_mode(FILE_WRITE_ATTRIBUTES)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(dir)
            .unwrap()
    };
    file.set_times(std::fs::FileTimes::new().set_modified(when))
        .unwrap();
}

/// A home directory with something from every AI tool Prune knows about.
fn populated_home(platform: &SandboxPlatform) -> PathBuf {
    let home = platform.home();
    // Hugging Face: one folder per model; the lock folder and version file are not models.
    write(
        &home.join(".cache/huggingface/hub/models--org--small/blobs/a"),
        100,
    );
    write(
        &home.join(".cache/huggingface/hub/datasets--org--set/blobs/b"),
        50,
    );
    write(
        &home.join(".cache/huggingface/hub/.locks/models--org--small/x.lock"),
        1,
    );
    write(&home.join(".cache/huggingface/hub/version.txt"), 1);
    // LM Studio: publisher/model.
    write(
        &home.join(".lmstudio/models/publisher/model-q4/model.gguf"),
        70,
    );
    write(&home.join(".cache/torch/hub/checkpoints/resnet.pth"), 30);
    write(&home.join(".cache/whisper/base.pt"), 20);
    // Anything else in ~/.cache is somebody's; a runtime and the folders other providers
    // report are left out of it.
    write(&home.join(".cache/some-tool/index.db"), 10);
    write(&home.join(".cache/codex-runtimes/node/bin/node"), 5);
    write(&home.join(".cache/pip/http/x"), 5);
    // Ollama, Codex and Claude: what cannot be downloaded again.
    write(&home.join(".ollama/models/blobs/sha256-abc"), 300);
    write(&home.join(".codex/sessions/2026/11/rollout.jsonl"), 40);
    write(&home.join(".codex/archived_sessions/old.jsonl"), 4);
    write(&home.join(".codex/generated_images/cat.png"), 6);
    write(&home.join(".codex/worktrees/a1b2/repo/src/lib.rs"), 8);
    write(&home.join(".codex/thread_history_3.sqlite"), 9);
    write(&home.join(".codex/config.toml"), 1);
    write(
        &home.join(".claude/projects/-Users-me-app/session.jsonl"),
        60,
    );
    write(
        &home.join("Library/Application Support/Claude/vm_bundles/claudevm.bundle/disk.img"),
        500,
    );
    home
}

#[test]
fn finds_each_tool_where_it_keeps_its_data() {
    let dir = tempfile::tempdir().unwrap();
    let platform = SandboxPlatform::new(dir.path());
    platform.prepare().unwrap();
    let home = populated_home(&platform);

    let session = scan(&engine(platform));
    let found = by_provider(&session, &home);

    let expect = |id: &str, paths: &[&str]| {
        assert_eq!(
            found.get(id).cloned().unwrap_or_default(),
            paths.iter().map(|p| p.to_string()).collect::<Vec<_>>(),
            "{id}"
        );
    };
    expect(
        "ai_hugging_face",
        &[
            ".cache/huggingface/hub/datasets--org--set",
            ".cache/huggingface/hub/models--org--small",
        ],
    );
    expect("ai_lm_studio", &[".lmstudio/models/publisher/model-q4"]);
    expect("ai_pytorch_hub", &[".cache/torch"]);
    expect("ai_whisper", &[".cache/whisper"]);
    expect("ai_dot_cache", &[".cache/some-tool"]);
    expect("ai_ollama", &[".ollama/models"]);
    expect(
        "ai_codex_history",
        &[
            ".codex/archived_sessions",
            ".codex/generated_images",
            ".codex/sessions",
            ".codex/thread_history_3.sqlite",
            ".codex/worktrees/a1b2",
        ],
    );
    expect(
        "ai_claude_code_history",
        &[".claude/projects/-Users-me-app"],
    );
    expect(
        "ai_claude_desktop_vm",
        &["Library/Application Support/Claude/vm_bundles"],
    );
    assert!(
        !found.contains_key("ai_codex_scratch"),
        "there is no scratch folder"
    );
}

#[test]
fn models_wait_for_a_person_and_history_is_never_removable() {
    let dir = tempfile::tempdir().unwrap();
    let platform = SandboxPlatform::new(dir.path());
    platform.prepare().unwrap();
    let home = populated_home(&platform);
    let engine = engine(platform);
    let session = scan(&engine);

    for t in targets(&session) {
        let expected = match t.provider_id.as_str() {
            "ai_hugging_face" | "ai_lm_studio" | "ai_pytorch_hub" | "ai_whisper"
            | "ai_codex_scratch" => RiskLevel::Medium,
            "ai_dot_cache" => RiskLevel::High,
            "ai_ollama"
            | "ai_codex_history"
            | "ai_claude_code_history"
            | "ai_claude_desktop_vm" => RiskLevel::Protected,
            other => panic!("unexpected provider {other}"),
        };
        assert_eq!(t.risk, expected, "{}", t.path);
    }
    assert!(session
        .results
        .iter()
        .all(|r| r.category == Category::AiData));

    // What is protected is counted, and kept out of what can be reclaimed.
    let protected: u64 = targets(&session)
        .iter()
        .filter(|t| t.risk == RiskLevel::Protected)
        .map(|t| t.size_bytes)
        .sum();
    assert_eq!(protected, 300 + 40 + 4 + 6 + 8 + 9 + 60 + 500);
    assert_eq!(session.protected_bytes, protected);
    assert_eq!(session.reclaimable_bytes(), session.total_bytes - protected);

    // Asking for everything still refuses every protected target, by any route.
    let all: Vec<String> = targets(&session).iter().map(|t| t.id.clone()).collect();
    let plan = engine.plan(&session, &all, DeleteMode::Permanent);
    let blocked: Vec<_> = plan.blocked.iter().map(|b| b.reason.as_str()).collect();
    // Ollama, five from Codex, Claude Code, the Claude virtual machine.
    assert_eq!(blocked.len(), 8, "{:?}", plan.blocked);
    assert!(blocked.iter().all(|r| *r == "protected"));
    assert!(plan.targets.iter().all(|t| t.risk != RiskLevel::Protected));

    let result = engine.execute(&plan, None, &|_| {}).unwrap();
    assert!(result.failed.is_empty(), "{:?}", result.failed);
    assert!(!home
        .join(".cache/huggingface/hub/models--org--small")
        .exists());
    assert!(!home.join(".cache/some-tool").exists());
    for kept in [
        ".ollama/models/blobs/sha256-abc",
        ".codex/sessions/2026/11/rollout.jsonl",
        ".codex/worktrees/a1b2/repo/src/lib.rs",
        ".codex/thread_history_3.sqlite",
        ".codex/config.toml",
        ".claude/projects/-Users-me-app/session.jsonl",
        "Library/Application Support/Claude/vm_bundles/claudevm.bundle/disk.img",
        // Not a model, and not reported.
        ".cache/huggingface/hub/version.txt",
        ".cache/codex-runtimes/node/bin/node",
        ".cache/pip/http/x",
    ] {
        assert!(home.join(kept).exists(), "{kept} must survive");
    }
}

#[test]
fn codex_scratch_leaves_out_what_a_task_may_still_be_using() {
    let dir = tempfile::tempdir().unwrap();
    let platform = SandboxPlatform::new(dir.path());
    platform.prepare().unwrap();
    let home = platform.home();
    let tmp = home.join(".codex/tmp");
    write(&tmp.join("old-build/out.log"), 10);
    write(&tmp.join("running-task/out.log"), 10);
    age(&tmp.join("old-build"), Duration::from_secs(3 * 86_400));

    let session = scan(&engine(platform));
    assert_eq!(
        by_provider(&session, &home).get("ai_codex_scratch"),
        Some(&vec![".codex/tmp/old-build".to_string()])
    );
}

#[test]
fn nothing_is_reported_where_no_ai_tool_has_been() {
    let dir = tempfile::tempdir().unwrap();
    let platform = SandboxPlatform::new(dir.path());
    platform.prepare().unwrap();
    let session = scan(&engine(platform));
    assert!(targets(&session).is_empty());
    assert!(session.results.iter().all(|r| r.issues.is_empty()));
    assert_eq!(session.protected_bytes, 0);
}
