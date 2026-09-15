//! End-to-end tests of the real IPC surface on Tauri's mock runtime.
//!
//! These exercise the actual `#[tauri::command]` functions the desktop app calls — argument
//! deserialization, state access, the async runtime and the JSON responses — which the
//! frontend's browser mock cannot cover.
//!
//! Everything happens inside a temporary directory. The only real system data touched is
//! read-only (provider catalogue, system info, startup items).

use std::path::Path;

use prune_lib::{register_commands, AppState};
use serde_json::{json, Value};
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{Manager, WebviewWindow, WebviewWindowBuilder};

fn build_app(data_dir: &Path) -> (tauri::App<MockRuntime>, WebviewWindow<MockRuntime>) {
    let state = AppState::new(data_dir).expect("app state");
    let app = register_commands(mock_builder())
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

    // Revealing something outside the user's own directories is refused.
    let err = invoke(&webview, "fs_reveal", json!({ "path": "/etc/hosts" })).unwrap_err();
    assert_eq!(err["code"], "invalid_path");
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
