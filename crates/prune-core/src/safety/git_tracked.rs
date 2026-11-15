//! Does a build folder hold anything a Git repository tracks?
//!
//! A folder called `dist` next to a `package.json` is usually rebuildable output, but not
//! always: every JavaScript GitHub Action commits its `dist/`, sites commit `build/`, and a
//! vendored library can sit in `out/`. Moving those to the Trash takes committed work with
//! them. So before a build folder inside a repository is offered, Git is asked whether it
//! tracks anything in it, and the folder is kept when it does — or when the question could not
//! be answered.
//!
//! The question is `git ls-files`, which reads the index and nothing else: it runs no hooks,
//! filters or network operations, takes no lock (`GIT_OPTIONAL_LOCKS=0`), and starts no
//! file-system monitor (`core.fsmonitor=false`), so asking leaves no process behind. It is
//! asked from the folder's parent with the folder's own name, so Git resolves the repository and
//! the spelling of the path itself, including macOS's two ways of writing the same accented or
//! Hangul name.

use std::ffi::OsString;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

/// A repository on a slow or unresponsive volume must not hold the scan.
const TIMEOUT: Duration = Duration::from_secs(10);
/// Enough of Git's complaint to say what went wrong.
const STDERR_LIMIT: usize = 1024;

/// What Git says about one folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitEvidence {
    /// Not inside a repository: nothing to ask.
    NotInRepository,
    /// Inside a repository, which tracks nothing in it.
    Untracked,
    /// The repository tracks at least one file in it (committed or staged), or a submodule.
    Tracked,
    /// The folder is a repository of its own.
    Repository,
    /// The question could not be answered; the reason says why.
    Unknown(String),
}

impl GitEvidence {
    /// Whether the folder may be offered for removal on this evidence.
    pub fn allows_removal(&self) -> bool {
        matches!(self, Self::NotInRepository | Self::Untracked)
    }
}

/// The nearest directory at or above `dir` that holds a `.git` (a directory, or the file a
/// worktree or submodule has instead).
pub fn repository_of(dir: &Path) -> Option<&Path> {
    dir.ancestors().find(|a| a.join(".git").exists())
}

/// Asks `git` whether the repository containing `folder` tracks anything inside it.
pub fn inspect(git: Option<&Path>, folder: &Path, cancel: &AtomicBool) -> GitEvidence {
    if folder.join(".git").exists() {
        return GitEvidence::Repository;
    }
    let (Some(parent), Some(name)) = (folder.parent(), folder.file_name()) else {
        return GitEvidence::Unknown("the folder has no parent to ask from".into());
    };
    if repository_of(parent).is_none() {
        return GitEvidence::NotInRepository;
    }
    let Some(git) = git else {
        return GitEvidence::Unknown("Git was not found".into());
    };
    if cancel.load(Ordering::Relaxed) {
        return GitEvidence::Unknown("the scan was cancelled".into());
    }
    match tracks_anything(git, parent, name) {
        Ok(true) => GitEvidence::Tracked,
        Ok(false) => GitEvidence::Untracked,
        Err(reason) => GitEvidence::Unknown(reason),
    }
}

/// `git ls-files` for one name in `parent`. Any output at all means something is tracked, so
/// it stops reading at the first byte rather than listing a vendored tree of ten thousand files.
fn tracks_anything(git: &Path, parent: &Path, name: &std::ffi::OsStr) -> Result<bool, String> {
    // `literal`: a name like `out[1]` is that name, not a pattern. `icase`: on a disk that
    // ignores case, `Dist` and `dist` are the same folder, so a tracked `Dist/` counts.
    let mut pathspec = OsString::from(":(literal,icase)");
    pathspec.push(name);
    let mut command = Command::new(git);
    command
        .args([
            "-c",
            "core.fsmonitor=false",
            "ls-files",
            "-z",
            "--cached",
            "--",
        ])
        .arg(pathspec)
        .current_dir(parent)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // A console program started from a windowed app opens a console window of its own.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("could not run Git: {e}"))?;

    let mut stdout = child.stdout.take().expect("stdout is piped");
    let mut stderr = child.stderr.take().expect("stderr is piped");
    let (sent, received) = mpsc::channel();
    std::thread::spawn(move || {
        let mut first = [0u8; 1];
        let _ = sent.send(stdout.read(&mut first));
    });
    // Drained to the end so a long complaint cannot fill the pipe and stall Git; kept short.
    let complaint = std::thread::spawn(move || {
        let mut kept = Vec::new();
        let mut chunk = [0u8; 4096];
        while let Ok(n) = stderr.read(&mut chunk) {
            if n == 0 {
                break;
            }
            let room = STDERR_LIMIT.saturating_sub(kept.len());
            kept.extend_from_slice(&chunk[..n.min(room)]);
        }
        kept
    });

    let tracked = match received.recv_timeout(TIMEOUT) {
        Ok(Ok(n)) => n > 0,
        Ok(Err(e)) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("could not read Git's answer: {e}"));
        }
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "Git did not answer within {} seconds",
                TIMEOUT.as_secs()
            ));
        }
    };
    if tracked {
        // The answer is in; the rest of the list is not needed.
        let _ = child.kill();
        let _ = child.wait();
        return Ok(true);
    }
    // Nothing listed. That means "nothing tracked" only if Git also says it succeeded.
    let status = child
        .wait()
        .map_err(|e| format!("could not wait for Git: {e}"))?;
    let complaint = complaint.join().unwrap_or_default();
    if status.success() {
        return Ok(false);
    }
    let message = String::from_utf8_lossy(&complaint);
    let first = message
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("no explanation");
    Err(format!("Git failed ({status}): {first}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git() -> std::path::PathBuf {
        crate::platform::current()
            .git()
            .expect("these tests need git installed")
    }

    /// `git init` and `git add` only: nothing here depends on an identity, signing or a commit.
    fn run(dir: &Path, args: &[&str]) {
        let status = Command::new(git())
            .args(args)
            .current_dir(dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    }

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        run(dir.path(), &["init", "-q"]);
        dir
    }

    fn write(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"x").unwrap();
    }

    fn ask(folder: &Path) -> GitEvidence {
        inspect(Some(&git()), folder, &AtomicBool::new(false))
    }

    #[test]
    fn a_folder_outside_any_repository_needs_no_answer() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("app/dist/index.js"));
        assert_eq!(
            ask(&dir.path().join("app/dist")),
            GitEvidence::NotInRepository
        );
    }

    #[test]
    fn an_ignored_build_folder_is_untracked() {
        let repo = repo();
        write(&repo.path().join("web/dist/bundle.js"));
        write(&repo.path().join("web/src/main.ts"));
        run(repo.path(), &["add", "web/src/main.ts"]);
        assert_eq!(ask(&repo.path().join("web/dist")), GitEvidence::Untracked);
    }

    #[test]
    fn a_committed_dist_is_tracked_even_when_staged_only() {
        let repo = repo();
        write(&repo.path().join("action/dist/index.js"));
        run(repo.path(), &["add", "action/dist/index.js"]);
        assert_eq!(ask(&repo.path().join("action/dist")), GitEvidence::Tracked);
    }

    #[test]
    fn one_tracked_file_deep_inside_is_enough() {
        let repo = repo();
        write(&repo.path().join("build/a/b/c/kept.txt"));
        write(&repo.path().join("build/noise.o"));
        run(repo.path(), &["add", "-f", "build/a/b/c/kept.txt"]);
        assert_eq!(ask(&repo.path().join("build")), GitEvidence::Tracked);
    }

    #[test]
    fn a_name_is_taken_literally() {
        let repo = repo();
        write(&repo.path().join("out1/kept.txt"));
        write(&repo.path().join("out[1]/generated.txt"));
        run(repo.path(), &["add", "out1/kept.txt"]);
        // As a pattern, `out[1]` would match `out1`.
        assert_eq!(ask(&repo.path().join("out[1]")), GitEvidence::Untracked);
    }

    #[test]
    fn a_folder_that_is_a_repository_of_its_own_is_never_offered() {
        let repo = repo();
        let nested = repo.path().join("build");
        std::fs::create_dir_all(&nested).unwrap();
        run(&nested, &["init", "-q"]);
        let evidence = ask(&nested);
        assert_eq!(evidence, GitEvidence::Repository);
        assert!(!evidence.allows_removal());
    }

    #[test]
    fn without_git_a_folder_in_a_repository_is_unknown_and_kept() {
        let repo = repo();
        write(&repo.path().join("dist/index.js"));
        let evidence = inspect(None, &repo.path().join("dist"), &AtomicBool::new(false));
        assert!(matches!(evidence, GitEvidence::Unknown(_)), "{evidence:?}");
        assert!(!evidence.allows_removal());
    }

    #[test]
    fn a_failing_git_is_unknown_with_its_reason() {
        // A repository Git cannot read: the `.git` directory is there but empty.
        let broken = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(broken.path().join(".git")).unwrap();
        write(&broken.path().join("dist/index.js"));
        match ask(&broken.path().join("dist")) {
            GitEvidence::Unknown(reason) => assert!(reason.starts_with("Git failed"), "{reason}"),
            other => panic!("expected unknown, got {other:?}"),
        }
    }
}
