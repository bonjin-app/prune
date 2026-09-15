//! Docker, which on a developer machine is often the single largest thing on disk.
//!
//! Docker is the one target Prune cannot treat as files. Images, containers and build cache
//! live inside a disk image the daemon owns; deleting that file is a factory reset, not a
//! cleanup. So this module does not go through the safety pipeline at all — it asks the Docker
//! CLI what is reclaimable and, when the user agrees, asks Docker to reclaim it.
//!
//! Two consequences follow, and both are deliberate:
//!
//! - Nothing here is undoable. There is no trash for a pruned image, so the UI names the exact
//!   command before running it.
//! - Only the two conservative commands are offered. `docker system prune` without `-a` keeps
//!   images that are tagged and in use, and **never** passes `--volumes`, because volumes hold
//!   databases and other state a developer expects to survive a cleanup.

use std::process::Output;

use serde::{Deserialize, Serialize};

mod parse;

pub use parse::{parse_size, parse_usage};

/// Runs an external command. Injected so the tests can drive the whole flow without Docker.
pub trait CommandRunner: Send + Sync {
    fn run(&self, program: &str, args: &[&str]) -> std::io::Result<Output>;
}

/// Runs commands for real.
pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, program: &str, args: &[&str]) -> std::io::Result<Output> {
        std::process::Command::new(program).args(args).output()
    }
}

/// What kind of data Docker is holding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DockerKind {
    Images,
    Containers,
    Volumes,
    BuildCache,
}

impl DockerKind {
    pub fn label(self) -> &'static str {
        match self {
            DockerKind::Images => "Images",
            DockerKind::Containers => "Containers",
            DockerKind::Volumes => "Volumes",
            DockerKind::BuildCache => "Build cache",
        }
    }

    /// What removing this would cost, in plain terms.
    pub fn note(self) -> &'static str {
        match self {
            DockerKind::Images => "Unused images are re-pulled or rebuilt when needed.",
            DockerKind::Containers => "Stopped containers. Running ones are never touched.",
            DockerKind::Volumes => {
                "Volumes hold data such as databases. Prune never removes these."
            }
            DockerKind::BuildCache => "Layers from past builds. The next build is slower.",
        }
    }

    /// Whether Prune's own actions can reclaim this. Volumes are deliberately excluded.
    pub fn reclaimable_by_prune(self) -> bool {
        !matches!(self, DockerKind::Volumes)
    }
}

/// One row of `docker system df`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerEntry {
    pub kind: DockerKind,
    pub label: String,
    pub note: String,
    pub total_count: u64,
    pub active_count: u64,
    pub size_bytes: u64,
    pub reclaimable_bytes: u64,
    pub reclaimable_by_prune: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerUsage {
    pub entries: Vec<DockerEntry>,
    pub total_bytes: u64,
    /// Reclaimable by the actions Prune offers, so volumes are excluded.
    pub reclaimable_bytes: u64,
}

/// Whether Docker can be asked anything at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum DockerState {
    /// No `docker` on the path.
    NotInstalled,
    /// Installed, but the daemon did not answer.
    NotRunning {
        message: String,
    },
    Ready {
        usage: DockerUsage,
    },
}

/// The only two things Prune will ask Docker to remove.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DockerAction {
    /// `docker system prune -f`: stopped containers, unused networks, dangling images and
    /// build cache. Not `-a`, and never `--volumes`.
    SystemPrune,
    /// `docker builder prune -f`: build cache only.
    BuilderPrune,
}

impl DockerAction {
    /// The exact command line, so the UI can show it before running it.
    pub fn command(self) -> &'static [&'static str] {
        match self {
            DockerAction::SystemPrune => &["system", "prune", "--force"],
            DockerAction::BuilderPrune => &["builder", "prune", "--force"],
        }
    }

    pub fn display(self) -> String {
        format!("docker {}", self.command().join(" "))
    }

    pub fn label(self) -> &'static str {
        match self {
            DockerAction::SystemPrune => "Remove unused data",
            DockerAction::BuilderPrune => "Remove build cache",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            DockerAction::SystemPrune => {
                "Stopped containers, unused networks, dangling images and build cache. \
                 Images still in use and all volumes are kept."
            }
            DockerAction::BuilderPrune => {
                "Layers cached from past builds. Images and containers are untouched."
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerPruneResult {
    pub action: DockerAction,
    pub command: String,
    pub reclaimed_bytes: u64,
    /// What Docker printed, shown verbatim because Prune cannot verify it.
    pub output: String,
}

/// Asks Docker what it is holding.
///
/// Never fails: "no Docker" and "Docker is not running" are ordinary answers for a machine
/// that does not use it, not errors to show the user.
pub fn status(runner: &dyn CommandRunner) -> DockerState {
    let output = match runner.run("docker", &["system", "df", "--format", "{{json .}}"]) {
        Ok(output) => output,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return DockerState::NotInstalled,
        Err(e) => {
            return DockerState::NotRunning {
                message: e.to_string(),
            }
        }
    };
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return DockerState::NotRunning {
            message: if message.is_empty() {
                "Docker did not answer.".into()
            } else {
                message
            },
        };
    }
    DockerState::Ready {
        usage: parse_usage(&String::from_utf8_lossy(&output.stdout)),
    }
}

/// Asks Docker to reclaim space.
pub fn prune(runner: &dyn CommandRunner, action: DockerAction) -> crate::Result<DockerPruneResult> {
    let output = runner
        .run("docker", action.command())
        .map_err(|e| crate::PruneError::Other(format!("could not run Docker: {e}")))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(crate::PruneError::Other(if stderr.is_empty() {
            format!("{} failed", action.display())
        } else {
            stderr
        }));
    }
    Ok(DockerPruneResult {
        action,
        command: action.display(),
        reclaimed_bytes: parse::reclaimed_total(&stdout),
        output: stdout,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Builds an exit status without running anything. The raw value means different things on
    /// each platform, so only "zero" and "not zero" are used here.
    fn exit_status(success: bool) -> std::process::ExitStatus {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            std::process::ExitStatus::from_raw(if success { 0 } else { 256 })
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::ExitStatusExt;
            std::process::ExitStatus::from_raw(if success { 0 } else { 1 })
        }
    }

    /// Canned answer for one invocation.
    type Reply = Box<dyn Fn(&[&str]) -> std::io::Result<Output> + Send + Sync>;

    /// Records what was asked of Docker and replies with canned output.
    struct FakeDocker {
        calls: Mutex<Vec<Vec<String>>>,
        reply: Reply,
    }

    impl FakeDocker {
        fn new(reply: impl Fn(&[&str]) -> std::io::Result<Output> + Send + Sync + 'static) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                reply: Box::new(reply),
            }
        }

        fn calls(&self) -> Vec<Vec<String>> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl CommandRunner for FakeDocker {
        fn run(&self, program: &str, args: &[&str]) -> std::io::Result<Output> {
            self.calls.lock().unwrap().push(
                std::iter::once(program.to_string())
                    .chain(args.iter().map(|a| a.to_string()))
                    .collect(),
            );
            (self.reply)(args)
        }
    }

    fn ok_output(stdout: &str) -> std::io::Result<Output> {
        Ok(Output {
            status: exit_status(true),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        })
    }

    fn failed_output(stderr: &str) -> std::io::Result<Output> {
        Ok(Output {
            status: exit_status(false),
            stdout: Vec::new(),
            stderr: stderr.as_bytes().to_vec(),
        })
    }

    /// Real output from `docker system df --format "{{json .}}"`.
    const DF: &str = r#"{"Type":"Images","TotalCount":"24","Active":"3","Size":"12.4GB","Reclaimable":"9.1GB (73%)"}
{"Type":"Containers","TotalCount":"7","Active":"2","Size":"4.2GB","Reclaimable":"3.8GB (90%)"}
{"Type":"Local Volumes","TotalCount":"11","Active":"4","Size":"8.4GB","Reclaimable":"2.1GB (25%)"}
{"Type":"Build Cache","TotalCount":"132","Active":"0","Size":"6.8GB","Reclaimable":"6.8GB"}"#;

    #[test]
    fn a_machine_without_docker_is_not_an_error() {
        let fake = FakeDocker::new(|_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no docker",
            ))
        });
        assert_eq!(status(&fake), DockerState::NotInstalled);
    }

    #[test]
    fn a_stopped_daemon_reports_what_docker_said() {
        let fake = FakeDocker::new(|_| {
            failed_output("Cannot connect to the Docker daemon at unix:///var/run/docker.sock.")
        });
        match status(&fake) {
            DockerState::NotRunning { message } => assert!(message.contains("Cannot connect")),
            other => panic!("expected NotRunning, got {other:?}"),
        }
    }

    #[test]
    fn reads_what_docker_is_holding() {
        let fake = FakeDocker::new(|_| ok_output(DF));
        let DockerState::Ready { usage } = status(&fake) else {
            panic!("expected Ready");
        };

        assert_eq!(
            fake.calls()[0],
            vec!["docker", "system", "df", "--format", "{{json .}}"]
        );
        assert_eq!(usage.entries.len(), 4);

        let images = &usage.entries[0];
        assert_eq!(images.kind, DockerKind::Images);
        assert_eq!(images.total_count, 24);
        assert_eq!(images.active_count, 3);
        assert_eq!(images.size_bytes, 12_400_000_000);
        assert_eq!(images.reclaimable_bytes, 9_100_000_000);

        assert_eq!(usage.total_bytes, 31_800_000_000);
    }

    #[test]
    fn volumes_are_counted_but_never_offered() {
        let fake = FakeDocker::new(|_| ok_output(DF));
        let DockerState::Ready { usage } = status(&fake) else {
            panic!("expected Ready");
        };
        let volumes = usage
            .entries
            .iter()
            .find(|e| e.kind == DockerKind::Volumes)
            .expect("volumes row");
        assert_eq!(volumes.reclaimable_bytes, 2_100_000_000);
        assert!(!volumes.reclaimable_by_prune);
        // The headline number leaves the volumes out, because Prune will not remove them.
        assert_eq!(
            usage.reclaimable_bytes,
            9_100_000_000 + 3_800_000_000 + 6_800_000_000
        );
    }

    #[test]
    fn the_commands_are_the_conservative_ones() {
        // Guards against someone "improving" these into -a or --volumes.
        assert_eq!(
            DockerAction::SystemPrune.command(),
            &["system", "prune", "--force"]
        );
        assert_eq!(
            DockerAction::BuilderPrune.command(),
            &["builder", "prune", "--force"]
        );
        for action in [DockerAction::SystemPrune, DockerAction::BuilderPrune] {
            let command = action.command();
            assert!(
                !command.contains(&"--volumes"),
                "volumes must never be pruned"
            );
            assert!(
                !command.contains(&"-a"),
                "unused-image pruning must stay opt-in"
            );
            assert!(!command.contains(&"--all"));
        }
    }

    #[test]
    fn pruning_runs_the_command_and_reports_what_was_reclaimed() {
        let fake =
            FakeDocker::new(|_| ok_output("deleted: sha256:abc\n\nTotal reclaimed space: 9.1GB"));
        let result = prune(&fake, DockerAction::SystemPrune).unwrap();

        assert_eq!(
            fake.calls()[0],
            vec!["docker", "system", "prune", "--force"]
        );
        assert_eq!(result.reclaimed_bytes, 9_100_000_000);
        assert_eq!(result.command, "docker system prune --force");
        assert!(result.output.contains("deleted"));
    }

    #[test]
    fn a_failed_prune_surfaces_dockers_own_message() {
        let fake = FakeDocker::new(|_| failed_output("permission denied while trying to connect"));
        let err = prune(&fake, DockerAction::BuilderPrune).unwrap_err();
        assert!(err.to_string().contains("permission denied"), "{err}");
    }
}
