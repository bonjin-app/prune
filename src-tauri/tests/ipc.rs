//! End-to-end tests of the real IPC surface on Tauri's mock runtime.
//!
//! These exercise the actual `#[tauri::command]` functions the desktop app calls — argument
//! deserialization, state access, the async runtime and the JSON responses — which the
//! frontend's browser mock cannot cover.
//!
//! Everything happens inside a temporary directory. The only real system data touched is
//! read-only (provider catalogue, system info, startup items).
//!
//! Twenty-eight of the thirty commands are exercised here. The two that are not would act on
//! the machine running the tests: `docker_prune` would really prune Docker, and
//! `app_open_privacy_settings` would open System Settings in the tester's face. Both are
//! covered where they can be — Docker against a fake command runner in `prune-core`, and the
//! settings pane is a single spawn. If a new command is added, add it here rather than growing
//! that list.

use std::path::Path;

use prune_lib::{register_commands, register_plugins, AppState};
use serde_json::{json, Value};
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{Manager, WebviewWindow, WebviewWindowBuilder};

fn build_app(data_dir: &Path) -> (tauri::App<MockRuntime>, WebviewWindow<MockRuntime>) {
    let state = AppState::new(data_dir);
    let app = register_commands(register_plugins(mock_builder()))
        .build(mock_context(noop_assets()))
        .expect("mock app");
    app.manage(state);
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("webview");
    (app, webview)
}

fn invoke(webview: &WebviewWindow<MockRuntime>, cmd: &str, args: Value) -> Result<Value, Value> {
    let response = tauri::test::get_ipc_response(
        webview,
        InvokeRequest {
            cmd: cmd.into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "tauri://localhost".parse().unwrap(),
            body: tauri::ipc::InvokeBody::Json(args),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    );
    response.map(|body| body.deserialize::<Value>().expect("json response"))
}

fn ok(webview: &WebviewWindow<MockRuntime>, cmd: &str, args: Value) -> Value {
    invoke(webview, cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

/// Polls a command until it stops reporting `scan_running`.
fn wait_for(webview: &WebviewWindow<MockRuntime>, cmd: &str, args: Value) -> Value {
    for _ in 0..200 {
        match invoke(webview, cmd, args.clone()) {
            Ok(v) => return v,
            Err(e) => {
                let code = e.get("code").and_then(|c| c.as_str()).unwrap_or("");
                assert_eq!(code, "scan_running", "{cmd} failed: {e}");
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    }
    panic!("{cmd} never finished");
}

fn write(path: &Path, size: usize) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, vec![b'x'; size]).unwrap();
}

#[test]
fn read_only_commands_answer_over_ipc() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, webview) = build_app(dir.path());

    let meta = ok(&webview, "app_get_meta", json!({}));
    assert_eq!(meta["name"], "Prune");
    assert!(meta["operationLogPath"]
        .as_str()
        .unwrap()
        .ends_with("operations.jsonl"));

    let info = ok(&webview, "system_get_info", json!({}));
    assert!(!info["homeDir"].as_str().unwrap().is_empty());
    assert!(info["logicalCores"].as_u64().unwrap() >= 1);

    let snapshot = ok(&webview, "system_get_snapshot", json!({}));
    assert!(snapshot["memory"]["totalBytes"].as_u64().unwrap() > 0);

    let processes = ok(&webview, "system_list_processes", json!({ "limit": 5 }));
    assert!(processes.as_array().unwrap().len() <= 5);

    let providers = ok(&webview, "cleaner_list_providers", json!({}));
    let providers = providers.as_array().unwrap();
    assert!(providers.len() > 10);
    assert!(providers.iter().any(|p| p["id"] == "npm_cache"));

    // Read-only; a machine may legitimately have no startup items at all.
    let startup = ok(&webview, "startup_list", json!({}));
    assert!(startup.is_array());

    // Operation log starts empty.
    assert!(ok(&webview, "ops_list", json!({}))
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn disk_scan_preview_and_execute_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().join("appdata");
    let root = dir.path().join("workspace");
    write(&root.join("big/huge.bin"), 40_000);
    write(&root.join("big/nested/medium.bin"), 12_000);
    write(&root.join("small.txt"), 10);
    let (_app, webview) = build_app(&data_dir);

    let scan_id = ok(
        &webview,
        "disk_start_scan",
        json!({ "root": root.to_string_lossy() }),
    );
    let scan_id = scan_id.as_str().unwrap().to_string();

    let summary = wait_for(&webview, "disk_get_summary", json!({ "scanId": scan_id }));
    assert_eq!(summary["status"], "completed");
    assert_eq!(summary["totalBytes"].as_u64().unwrap(), 52_010);
    assert_eq!(summary["fileCount"].as_u64().unwrap(), 3);

    let node = ok(&webview, "disk_get_node", json!({ "scanId": scan_id }));
    let children = node["children"].as_array().unwrap();
    assert_eq!(children[0]["name"], "big");
    assert_eq!(children[0]["sizeBytes"].as_u64().unwrap(), 52_000);

    let deeper = ok(
        &webview,
        "disk_get_node",
        json!({ "scanId": scan_id, "path": root.join("big").to_string_lossy() }),
    );
    assert_eq!(deeper["breadcrumbs"].as_array().unwrap().len(), 1);

    let files = ok(
        &webview,
        "disk_large_files",
        json!({ "scanId": scan_id, "minBytes": 20_000, "limit": 10 }),
    );
    let files = files.as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["name"], "huge.bin");
    let target_id = files[0]["targetId"].as_str().unwrap().to_string();

    // Dry run: reports what would go, changes nothing.
    let plan = ok(
        &webview,
        "cleaner_preview",
        json!({ "scanId": scan_id, "targetIds": [target_id], "mode": "permanent" }),
    );
    assert_eq!(plan["totalBytes"].as_u64().unwrap(), 40_000);
    assert!(plan["blocked"].as_array().unwrap().is_empty());
    assert!(root.join("big/huge.bin").exists());
    let plan_id = plan["id"].as_str().unwrap().to_string();

    let result = ok(&webview, "cleaner_execute", json!({ "planId": plan_id }));
    assert_eq!(result["status"], "success");
    assert_eq!(result["removedBytes"].as_u64().unwrap(), 40_000);
    assert!(!root.join("big/huge.bin").exists());
    // Untouched.
    assert!(root.join("big/nested/medium.bin").exists());
    assert!(root.join("small.txt").exists());

    // The analysis forgets the removed file and the ancestors shrink.
    let after = ok(&webview, "disk_get_node", json!({ "scanId": scan_id }));
    assert_eq!(after["node"]["sizeBytes"].as_u64().unwrap(), 12_010);
    assert!(ok(
        &webview,
        "disk_large_files",
        json!({ "scanId": scan_id, "minBytes": 20_000 })
    )
    .as_array()
    .unwrap()
    .is_empty());

    // The operation was logged locally.
    let ops = ok(&webview, "ops_list", json!({}));
    let ops = ops.as_array().unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0]["removedBytes"].as_u64().unwrap(), 40_000);
    assert_eq!(ops[0]["mode"], "permanent");
}

#[test]
fn preview_refuses_targets_outside_the_scan() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("workspace");
    write(&root.join("file.bin"), 5_000);
    let (_app, webview) = build_app(&dir.path().join("appdata"));

    let scan_id = ok(
        &webview,
        "disk_start_scan",
        json!({ "root": root.to_string_lossy() }),
    );
    let scan_id = scan_id.as_str().unwrap().to_string();
    wait_for(&webview, "disk_get_summary", json!({ "scanId": scan_id }));

    let plan = ok(
        &webview,
        "cleaner_preview",
        json!({ "scanId": scan_id, "targetIds": ["not-a-real-target"], "mode": "permanent" }),
    );
    assert!(plan["targets"].as_array().unwrap().is_empty());
    assert_eq!(plan["blocked"].as_array().unwrap().len(), 1);
    assert_eq!(plan["totalBytes"].as_u64().unwrap(), 0);

    // Unknown ids never reach the filesystem.
    assert!(root.join("file.bin").exists());

    let err = invoke(&webview, "cleaner_execute", json!({ "planId": "nope" })).unwrap_err();
    assert_eq!(err["code"], "unknown_plan");

    let err = invoke(&webview, "disk_get_summary", json!({ "scanId": "nope" })).unwrap_err();
    assert_eq!(err["code"], "unknown_scan");
}

#[test]
fn disk_scan_and_reveal_reject_bad_paths() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, webview) = build_app(dir.path());

    let err = invoke(
        &webview,
        "disk_start_scan",
        json!({ "root": "relative/path" }),
    )
    .unwrap_err();
    assert_eq!(err["code"], "invalid_path");

    let missing = dir.path().join("does-not-exist");
    let err = invoke(
        &webview,
        "disk_start_scan",
        json!({ "root": missing.to_string_lossy() }),
    )
    .unwrap_err();
    assert_eq!(err["code"], "invalid_path");

    // Revealing something outside the user's own directories is refused — including by the
    // other name macOS has for the same file, which a plain prefix check would have allowed.
    for path in ["/etc/hosts", "/private/etc/hosts", "/usr/bin"] {
        match invoke(&webview, "fs_reveal", json!({ "path": path })) {
            Err(err) => assert_eq!(err["code"], "invalid_path", "{path}"),
            Ok(_) => panic!("{path} should not be revealable"),
        }
    }

    // The accepting case is covered in prune-core, where it can be checked without opening a
    // file manager window on whoever is running the tests.
}

#[test]
fn settings_report_the_folders_the_developer_scan_will_search() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, webview) = build_app(dir.path());

    let view = ok(&webview, "settings_get", json!({}));
    assert!(!view["homeDir"].as_str().unwrap().is_empty());
    // A fresh profile has nothing configured, so Prune is guessing.
    assert_eq!(view["projectRootsConfigured"], false);
    assert!(view["settings"]["projectRoots"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(view["effectiveProjectRoots"].is_array());
}

#[test]
fn settings_refuse_folders_prune_could_never_clean() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, webview) = build_app(dir.path());

    // Outside the home directory: every target found there would be blocked anyway.
    let err = invoke(
        &webview,
        "settings_set_project_roots",
        json!({ "roots": ["/usr/local"] }),
    )
    .unwrap_err();
    assert_eq!(err["code"], "invalid_path");
    assert!(err["message"].as_str().unwrap().contains("home folder"));

    let err = invoke(
        &webview,
        "settings_set_project_roots",
        json!({ "roots": ["relative/path"] }),
    )
    .unwrap_err();
    assert_eq!(err["code"], "invalid_path");

    let missing = dir.path().join("nope");
    let err = invoke(
        &webview,
        "settings_set_project_roots",
        json!({ "roots": [missing.to_string_lossy()] }),
    )
    .unwrap_err();
    assert_eq!(err["code"], "invalid_path");

    // A rejected list changes nothing.
    let view = ok(&webview, "settings_get", json!({}));
    assert!(view["settings"]["projectRoots"]
        .as_array()
        .unwrap()
        .is_empty());

    // An empty list is accepted and means "go back to guessing".
    let view = ok(
        &webview,
        "settings_set_project_roots",
        json!({ "roots": [] }),
    );
    assert_eq!(view["projectRootsConfigured"], false);
}

#[test]
fn only_the_most_recent_scans_are_kept_in_memory() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("workspace");
    write(&root.join("f.bin"), 100);
    let (_app, webview) = build_app(&dir.path().join("appdata"));

    // A disk analysis holds a node per directory, so keeping every one of them for the life of
    // the process is a slow leak. Run more than fit and check the oldest is gone.
    let mut ids = Vec::new();
    for _ in 0..5 {
        let id = ok(
            &webview,
            "disk_start_scan",
            json!({ "root": root.to_string_lossy() }),
        );
        let id = id.as_str().unwrap().to_string();
        wait_for(&webview, "disk_get_summary", json!({ "scanId": id }));
        ids.push(id);
    }

    let newest = ids.last().unwrap();
    assert!(
        invoke(&webview, "disk_get_summary", json!({ "scanId": newest })).is_ok(),
        "the scan being shown must still be there"
    );

    let oldest = &ids[0];
    let err = invoke(&webview, "disk_get_summary", json!({ "scanId": oldest })).unwrap_err();
    assert_eq!(
        err["code"], "unknown_scan",
        "the oldest analysis should have been released"
    );
}

#[test]
fn the_app_still_starts_when_its_data_directory_cannot_be_used() {
    // An unwritable or full data directory costs the user their history, not the application.
    let dir = tempfile::tempdir().unwrap();
    let blocked = dir.path().join("a-file");
    std::fs::write(&blocked, b"x").unwrap();
    let (_app, webview) = build_app(&blocked.join("data"));

    // The window is up and answering.
    assert_eq!(ok(&webview, "app_get_meta", json!({}))["name"], "Prune");
    // Reading the history is empty rather than an error.
    assert!(ok(&webview, "ops_list", json!({}))
        .as_array()
        .unwrap()
        .is_empty());
    // And scanning still works.
    assert!(
        ok(&webview, "cleaner_list_providers", json!({}))
            .as_array()
            .unwrap()
            .len()
            > 10
    );
}

#[test]
fn the_window_position_is_remembered_between_runs() {
    // Reopening where it was left is the least a desktop app can do, and the plugin that does
    // it is only useful if it is actually wired into the builder the application runs.
    use tauri_plugin_window_state::{AppHandleExt, StateFlags};

    let dir = tempfile::tempdir().unwrap();
    let (app, _webview) = build_app(&dir.path().join("appdata"));

    app.handle()
        .save_window_state(StateFlags::all())
        .expect("the window state plugin is registered");

    let file = app
        .path()
        .app_config_dir()
        .unwrap()
        .join(app.handle().filename());
    assert!(file.exists(), "expected window state at {}", file.display());

    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert!(
        saved.get("main").is_some(),
        "the main window should be in {saved}"
    );
}

#[test]
fn the_read_only_commands_added_late_answer_too() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, webview) = build_app(dir.path());

    // Permissions: a plain answer on any machine, granted or not.
    let permissions = ok(&webview, "app_get_permissions", json!({}));
    assert!(permissions["blocked"].is_array());
    assert!(["granted", "denied", "not_applicable"]
        .contains(&permissions["fullDiskAccess"].as_str().unwrap()));

    // A fresh profile has no providers.json, so nothing is wrong with it.
    assert!(ok(&webview, "providers_custom_issues", json!({}))
        .as_array()
        .unwrap()
        .is_empty());

    // Docker: "not installed" and "not running" are answers, not errors.
    let docker = ok(&webview, "docker_status", json!({}));
    assert!(["not_installed", "not_running", "ready"].contains(&docker["state"].as_str().unwrap()));

    // Applications: read-only, and every entry says whether Prune would touch it.
    let apps = ok(&webview, "apps_start_scan", json!({}));
    for app in apps.as_array().unwrap() {
        assert!(app["id"].is_string());
        assert!(app["isSystem"].is_boolean());
    }
}

#[test]
fn the_commands_that_change_something_refuse_what_they_do_not_know() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, webview) = build_app(dir.path());

    // Each of these would act on the real machine, so the test only drives the refusals. The
    // paths that do act are covered against a sandbox in prune-core.
    let cases = [
        (
            "apps_get_detail",
            json!({ "appId": "nothing-like-this" }),
            "unknown_app",
        ),
        (
            "apps_run_uninstaller",
            json!({ "appId": "nothing-like-this" }),
            "unknown_app",
        ),
        (
            "startup_set_enabled",
            json!({ "itemId": "nothing-like-this", "enabled": false }),
            "unknown_startup_item",
        ),
        (
            "cleaner_get_scan",
            json!({ "scanId": "no-such-scan" }),
            "unknown_scan",
        ),
        (
            "cleaner_cancel_scan",
            json!({ "scanId": "no-such-scan" }),
            "unknown_scan",
        ),
        (
            "disk_cancel_scan",
            json!({ "scanId": "no-such-scan" }),
            "unknown_scan",
        ),
        (
            "cleaner_execute",
            json!({ "planId": "no-such-plan" }),
            "unknown_plan",
        ),
    ];
    for (cmd, args, expected) in cases {
        match invoke(&webview, cmd, args) {
            Err(err) => assert_eq!(err["code"], expected, "{cmd}"),
            Ok(value) => panic!("{cmd} should have refused, got {value}"),
        }
    }

    // Stopping a process is refused by name, not by id, so the message has to say which.
    let err = invoke(
        &webview,
        "system_stop_process",
        json!({ "pid": 1, "mode": "force" }),
    )
    .unwrap_err();
    assert!(
        err["message"]
            .as_str()
            .unwrap()
            .contains("cannot be stopped"),
        "{err}"
    );
}

#[test]
fn a_scan_can_be_started_read_back_and_cancelled() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, webview) = build_app(dir.path());

    // Limited to one provider so the test does not walk the machine it runs on.
    let scan_id = ok(
        &webview,
        "cleaner_start_scan",
        json!({ "providerIds": ["maven_repo"] }),
    );
    let scan_id = scan_id.as_str().unwrap().to_string();

    // Cancelling is accepted whether or not it arrives before the scan finishes.
    ok(
        &webview,
        "cleaner_cancel_scan",
        json!({ "scanId": scan_id }),
    );

    let session = wait_for(&webview, "cleaner_get_scan", json!({ "scanId": scan_id }));
    assert!(["completed", "cancelled"].contains(&session["status"].as_str().unwrap()));
    // Only the provider that was asked for, whatever it found.
    for result in session["results"].as_array().unwrap() {
        assert_eq!(result["providerId"], "maven_repo");
    }
}
