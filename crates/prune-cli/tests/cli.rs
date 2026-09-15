//! Drives the command line against a sandboxed home directory.
//!
//! The point is the parts a unit test cannot reach: which targets `clean` is willing to select
//! on its own, that it removes nothing without `--yes`, and that what it does remove is written
//! to the operation log.

use clap::Parser;
use prune_cli::{Cli, EXIT_NOTHING, EXIT_OK};
use prune_core::ops::OperationLog;
use prune_core::platform::sandbox::SandboxPlatform;
use prune_core::providers::ProviderRegistry;
use prune_core::PruneEngine;

struct Harness {
    _dir: tempfile::TempDir,
    sandbox: SandboxPlatform,
    engine: PruneEngine,
    log: OperationLog,
}

fn harness() -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let sandbox = SandboxPlatform::new(dir.path());
    sandbox.prepare().unwrap();

    // A safe cache, a low-risk project artifact, and data that must survive everything.
    sandbox
        .write("Library/Caches/com.example.app/blob.bin", 4_000)
        .unwrap();
    sandbox.write("Library/Logs/app.log", 500).unwrap();
    sandbox.write("Projects/web/package.json", 2).unwrap();
    sandbox
        .write("Projects/web/node_modules/left-pad/index.js", 3_000)
        .unwrap();
    sandbox.write("Documents/thesis.txt", 100).unwrap();
    sandbox.write(".ssh/id_ed25519", 64).unwrap();

    let engine =
        PruneEngine::with_parts(Box::new(sandbox.clone()), ProviderRegistry::with_defaults());
    let log = OperationLog::open(&dir.path().join("logdir")).unwrap();
    Harness {
        _dir: dir,
        sandbox,
        engine,
        log,
    }
}

/// Runs `prune …` and returns `(exit code, stdout)`.
fn run(h: &Harness, args: &[&str]) -> (i32, String) {
    let cli = Cli::parse_from(std::iter::once("prune").chain(args.iter().copied()));
    let mut out: Vec<u8> = Vec::new();
    let code = prune_cli::run(&h.engine, Some(&h.log), &cli, &mut out).unwrap();
    (code, String::from_utf8(out).unwrap())
}

#[test]
fn scan_reports_what_it_found_without_touching_anything() {
    let h = harness();
    let (code, out) = run(&h, &["scan"]);

    assert_eq!(code, EXIT_OK);
    assert!(out.contains("Application Caches"), "{out}");
    assert!(out.contains("node_modules"), "{out}");
    assert!(out.contains("Total"), "{out}");
    // Nothing that belongs to the user is even mentioned.
    assert!(!out.contains("thesis.txt"), "{out}");
    assert!(!out.contains("id_ed25519"), "{out}");
    assert!(h
        .sandbox
        .home()
        .join("Library/Caches/com.example.app")
        .exists());
}

#[test]
fn scan_can_be_limited_to_one_provider_and_emits_json() {
    let h = harness();
    let (code, out) = run(&h, &["scan", "--only", "user_cache", "--json"]);

    assert_eq!(code, EXIT_OK);
    let session: serde_json::Value = serde_json::from_str(&out).unwrap();
    let providers: Vec<&str> = session["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["providerId"].as_str().unwrap())
        .collect();
    assert_eq!(providers, vec!["user_cache"]);
    // JSON mode prints nothing but JSON, so it can be piped.
    assert!(!out.contains("Scanning"), "{out}");
}

#[test]
fn clean_is_a_dry_run_unless_asked() {
    let h = harness();
    let cache = h.sandbox.home().join("Library/Caches/com.example.app");
    let (code, out) = run(&h, &["clean"]);

    assert_eq!(code, EXIT_OK);
    assert!(out.contains("Dry run"), "{out}");
    assert!(out.contains("Would remove"), "{out}");
    assert!(cache.exists(), "a dry run must not remove anything");
    assert!(
        h.log.list(10).unwrap().is_empty(),
        "a dry run is not an operation"
    );
}

#[test]
fn clean_takes_safe_items_and_leaves_low_risk_ones_alone() {
    let h = harness();
    let cache = h.sandbox.home().join("Library/Caches/com.example.app");
    let node_modules = h.sandbox.home().join("Projects/web/node_modules");

    let (code, out) = run(&h, &["clean", "--permanent", "--yes"]);

    assert_eq!(code, EXIT_OK, "{out}");
    assert!(out.contains("Removed"), "{out}");
    assert!(!cache.exists(), "the safe cache should be gone");
    assert!(
        node_modules.exists(),
        "node_modules is low risk and must not go without --include-low"
    );
    assert!(h.sandbox.home().join("Documents/thesis.txt").exists());
    assert!(h.sandbox.home().join(".ssh/id_ed25519").exists());

    let records = h.log.list(10).unwrap();
    assert_eq!(records.len(), 1);
    assert!(records[0].removed_bytes >= 4_000);
}

#[test]
fn include_low_also_takes_project_artifacts() {
    let h = harness();
    let node_modules = h.sandbox.home().join("Projects/web/node_modules");

    let (code, out) = run(
        &h,
        &[
            "clean",
            "--include-low",
            "--permanent",
            "--yes",
            "--only",
            "project_artifacts",
        ],
    );

    assert_eq!(code, EXIT_OK, "{out}");
    assert!(!node_modules.exists(), "{out}");
    // The project itself is untouched; only the regenerable folder went.
    assert!(h.sandbox.home().join("Projects/web/package.json").exists());
}

#[test]
fn clean_says_so_when_there_is_nothing_to_do() {
    let h = harness();
    let (code, out) = run(&h, &["clean", "--only", "maven_repo", "--yes"]);

    assert_eq!(code, EXIT_NOTHING);
    assert!(out.contains("Nothing to remove"), "{out}");
}

#[test]
fn disk_analyses_a_directory_read_only() {
    let h = harness();
    let projects = h.sandbox.home().join("Projects");
    let (code, out) = run(&h, &["disk", projects.to_str().unwrap(), "--large", "0"]);

    assert_eq!(code, EXIT_OK);
    assert!(out.contains("web"), "{out}");
    assert!(out.contains("File types"), "{out}");
    assert!(projects.join("web/package.json").exists());
}

#[test]
fn disk_refuses_a_path_that_is_not_a_directory() {
    let h = harness();
    let file = h.sandbox.home().join("Documents/thesis.txt");
    let (code, out) = run(&h, &["disk", file.to_str().unwrap()]);

    assert_eq!(code, prune_cli::EXIT_ERROR);
    assert!(out.contains("not a directory"), "{out}");
}

#[test]
fn providers_and_status_describe_the_machine() {
    let h = harness();

    let (code, out) = run(&h, &["providers"]);
    assert_eq!(code, EXIT_OK);
    assert!(out.contains("npm_cache"), "{out}");
    assert!(out.contains("project_artifacts"), "{out}");

    let (code, out) = run(&h, &["status"]);
    assert_eq!(code, EXIT_OK);
    assert!(
        out.contains(&h.sandbox.home().display().to_string()),
        "{out}"
    );
}

#[test]
fn log_shows_what_was_removed() {
    let h = harness();
    let (_, _) = run(&h, &["clean", "--permanent", "--yes"]);

    let (code, out) = run(&h, &["log"]);
    assert_eq!(code, EXIT_OK);
    assert!(out.contains("permanent"), "{out}");

    let (code, out) = run(&h, &["log", "--json"]);
    assert_eq!(code, EXIT_OK);
    let records: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(records.as_array().unwrap().len(), 1);
}
