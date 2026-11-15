//! `prune protect` against a settings folder in a temporary directory.
//!
//! What matters is what lands in the settings file the desktop app reads: that a bad path saves
//! nothing, that other settings survive an edit, and that an unreadable file is left alone.

use std::path::{Path, PathBuf};

use clap::Parser;
use prune_cli::{Cli, Command, EXIT_ERROR, EXIT_NOTHING, EXIT_OK};
use prune_core::settings::Settings;

struct Harness {
    _root: tempfile::TempDir,
    settings: PathBuf,
    home: PathBuf,
}

fn harness() -> Harness {
    let root = tempfile::tempdir().unwrap();
    // Canonical, so the stored forms are predictable (`/var` is `/private/var` on macOS).
    let base = std::fs::canonicalize(root.path()).unwrap();
    let settings = base.join("settings");
    let home = base.join("home");
    std::fs::create_dir_all(home.join("code").join("site").join("dist")).unwrap();
    std::fs::create_dir_all(home.join("Documents").join("keys")).unwrap();
    Harness {
        _root: root,
        settings,
        home,
    }
}

fn run(h: &Harness, args: &[&str]) -> (i32, String) {
    let cli = Cli::parse_from(std::iter::once("prune").chain(args.iter().copied()));
    let Command::Protect { action } = &cli.command else {
        panic!("not a protect command");
    };
    let mut out = Vec::new();
    let code = prune_cli::protect::run(
        Some(&h.settings),
        Some(&h.home),
        &h.home.join("code"),
        action.as_ref(),
        cli.json,
        &mut out,
    )
    .unwrap();
    (code, String::from_utf8(out).unwrap())
}

fn stored(h: &Harness) -> Vec<PathBuf> {
    Settings::load_checked(&h.settings).settings.protected_paths
}

#[test]
fn an_empty_list_says_how_to_add_to_it() {
    let h = harness();
    let (code, out) = run(&h, &["protect"]);
    assert_eq!(code, EXIT_NOTHING);
    assert!(out.contains("prune protect add"), "{out}");
}

#[test]
fn adds_home_relative_and_relative_paths_and_keeps_other_settings() {
    let h = harness();
    Settings {
        project_roots: vec![h.home.join("code")],
        protected_paths: vec![],
    }
    .save(&h.settings)
    .unwrap();

    let (code, out) = run(&h, &["protect", "add", "~/Documents/keys", "site/dist/"]);
    assert_eq!(code, EXIT_OK, "{out}");
    let expected = vec![
        h.home.join("Documents").join("keys"),
        h.home.join("code").join("site").join("dist"),
    ];
    assert_eq!(stored(&h), expected);
    assert_eq!(
        Settings::load(&h.settings).project_roots,
        vec![h.home.join("code")]
    );

    // Adding again changes nothing and repeats nothing.
    let (code, _) = run(&h, &["protect", "add", "~/Documents/keys"]);
    assert_eq!(code, EXIT_OK);
    assert_eq!(stored(&h), expected);

    let (_, out) = run(&h, &["protect", "--json"]);
    let value: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(value["protectedPaths"].as_array().unwrap().len(), 2);
}

#[test]
fn one_bad_path_saves_nothing() {
    let h = harness();
    let (code, out) = run(&h, &["protect", "add", "~/Documents/keys", "~/typo"]);
    assert_eq!(code, EXIT_ERROR);
    assert!(out.contains("~/typo: folder does not exist"), "{out}");
    assert!(out.contains("Nothing was changed"), "{out}");
    assert!(stored(&h).is_empty());
}

#[test]
fn removes_by_any_spelling_even_once_the_path_is_gone() {
    let h = harness();
    run(
        &h,
        &["protect", "add", "~/Documents/keys", "~/code/site/dist"],
    );

    std::fs::remove_dir_all(h.home.join("Documents").join("keys")).unwrap();
    let (code, out) = run(&h, &["protect"]);
    assert_eq!(code, EXIT_OK);
    assert!(out.contains("(missing now)"), "{out}");

    let (code, out) = run(
        &h,
        &["protect", "remove", "~/Documents/keys", "./site/dist"],
    );
    assert_eq!(code, EXIT_OK, "{out}");
    assert!(stored(&h).is_empty());
}

#[test]
fn removing_something_not_on_the_list_changes_nothing() {
    let h = harness();
    run(&h, &["protect", "add", "~/Documents/keys"]);
    let (code, out) = run(
        &h,
        &["protect", "remove", "~/Documents/keys", "~/code/site/dist"],
    );
    assert_eq!(code, EXIT_ERROR);
    assert!(out.contains("is not on the list"), "{out}");
    assert_eq!(stored(&h), vec![h.home.join("Documents").join("keys")]);
}

#[test]
fn an_unreadable_settings_file_is_left_exactly_as_it_is() {
    let h = harness();
    std::fs::create_dir_all(&h.settings).unwrap();
    let file = h.settings.join("settings.json");
    let broken = b"{ \"protectedPaths\": [\"/keep\"";
    std::fs::write(&file, broken).unwrap();

    for args in [
        &["protect"][..],
        &["protect", "add", "~/Documents/keys"],
        &["protect", "remove", "/keep"],
    ] {
        let (code, out) = run(&h, args);
        assert_eq!(code, EXIT_ERROR, "{args:?}");
        assert!(out.contains("Nothing was changed"), "{out}");
    }
    assert_eq!(std::fs::read(&file).unwrap(), broken);
    assert_eq!(std::fs::read_dir(&h.settings).unwrap().count(), 1);
}

#[test]
fn without_a_settings_folder_it_refuses() {
    let mut out = Vec::new();
    let code = prune_cli::protect::run(None, None, Path::new("/"), None, false, &mut out).unwrap();
    assert_eq!(code, EXIT_ERROR);
}
