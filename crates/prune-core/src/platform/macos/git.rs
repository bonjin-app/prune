//! Where to find a `git` on macOS that will not ask to install anything.
//!
//! `/usr/bin/git` is a stub. Without the Command Line Tools it does not run git: it opens a
//! dialog offering to install them, which a cleanup scan must never do. So it is used only when
//! `xcode-select -p` (which never prompts) says developer tools are present, and then through
//! their own directory rather than the stub. Homebrew's and GitHub Desktop's copies are real
//! programs and safe to run directly.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const XCODE_SELECT: &str = "/usr/bin/xcode-select";
const STUBS: &[&str] = &["/usr/bin/git"];
const KNOWN: &[&str] = &[
    "/opt/homebrew/bin/git",
    "/usr/local/bin/git",
    "/Applications/GitHub Desktop.app/Contents/Resources/app/git/bin/git",
];

pub fn locate() -> Option<PathBuf> {
    // A git the user put on PATH themselves comes first, as long as it is not the stub.
    let on_path = std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .filter(|dir| dir.is_absolute())
            .map(|dir| dir.join("git"))
            .find(|git| git.is_file() && !STUBS.iter().any(|stub| git == Path::new(stub)))
    });
    on_path
        .or_else(|| KNOWN.iter().map(PathBuf::from).find(|git| git.is_file()))
        .or_else(developer_tools_git)
}

/// The git inside the active developer directory, if there is one.
fn developer_tools_git() -> Option<PathBuf> {
    let out = Command::new(XCODE_SELECT)
        .arg("-p")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let dir = PathBuf::from(String::from_utf8(out.stdout).ok()?.trim());
    let git = dir.join("usr/bin/git");
    git.is_file().then_some(git)
}
