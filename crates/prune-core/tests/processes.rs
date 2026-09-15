//! Stopping a process for real.
//!
//! The classifier is unit-tested, but the part that matters is the whole path: a process
//! Prune started must actually stop, and one it must never touch has to be refused by the same
//! call the UI makes. The only process these tests stop is one they started themselves.

use std::process::{Child, Command};
use std::time::{Duration, Instant};

use prune_core::models::StopMode;
use prune_core::system::SystemMonitor;

/// A long-running child that is cleaned up even if an assertion fails.
struct Sleeper(Child);

impl Sleeper {
    fn spawn() -> Self {
        let child = if cfg!(windows) {
            Command::new("cmd")
                .args(["/C", "ping -n 60 127.0.0.1 > nul"])
                .spawn()
        } else {
            Command::new("sleep").arg("60").spawn()
        };
        Sleeper(child.expect("failed to start the test process"))
    }

    fn pid(&self) -> u32 {
        self.0.id()
    }

    /// `true` once the process is no longer running.
    fn has_exited(&mut self) -> bool {
        matches!(self.0.try_wait(), Ok(Some(_)))
    }
}

impl Drop for Sleeper {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn monitor() -> SystemMonitor {
    SystemMonitor::new(&dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/")))
}

fn wait_for_exit(sleeper: &mut Sleeper, within: Duration) -> bool {
    let deadline = Instant::now() + within;
    while Instant::now() < deadline {
        if sleeper.has_exited() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    sleeper.has_exited()
}

#[test]
fn asking_a_process_to_stop_actually_stops_it() {
    let monitor = monitor();
    let mut sleeper = Sleeper::spawn();

    monitor
        .stop_process(sleeper.pid(), StopMode::Ask)
        .expect("a process we started should be stoppable");

    assert!(
        wait_for_exit(&mut sleeper, Duration::from_secs(5)),
        "the process was still running after being asked to stop"
    );
}

#[test]
fn forcing_a_process_to_stop_actually_stops_it() {
    let monitor = monitor();
    let mut sleeper = Sleeper::spawn();

    monitor
        .stop_process(sleeper.pid(), StopMode::Force)
        .expect("a process we started should be stoppable");

    assert!(wait_for_exit(&mut sleeper, Duration::from_secs(5)));
}

#[test]
fn the_init_process_is_refused() {
    let monitor = monitor();
    // pid 1 is launchd on macOS, systemd on Linux, and the System process on Windows.
    let err = monitor
        .stop_process(1, StopMode::Force)
        .expect_err("pid 1 must never be stoppable");
    assert!(
        err.to_string().contains("cannot be stopped"),
        "unexpected error: {err}"
    );
}

#[test]
fn prune_refuses_to_stop_itself() {
    let monitor = monitor();
    let err = monitor
        .stop_process(std::process::id(), StopMode::Force)
        .expect_err("stopping ourselves must be refused");
    assert!(err.to_string().contains("this is Prune"), "{err}");
}

#[test]
fn an_unknown_process_id_is_an_error_not_a_silent_success() {
    let monitor = monitor();
    // A pid that cannot exist: the maximum is well below this on every supported platform.
    let err = monitor
        .stop_process(u32::MAX, StopMode::Ask)
        .expect_err("a missing process must be reported");
    assert!(err.to_string().contains("no process"), "{err}");
}

#[test]
fn the_process_list_marks_what_can_and_cannot_be_stopped() {
    let monitor = monitor();
    let sleeper = Sleeper::spawn();
    // Enough entries that our own child is somewhere in the list.
    let processes = monitor.processes(2000);

    let ours = processes.iter().find(|p| p.pid == sleeper.pid());
    if let Some(ours) = ours {
        assert!(
            ours.can_terminate,
            "our own child should be stoppable, got {:?}",
            ours.protected_reason
        );
    }

    let prune = processes.iter().find(|p| p.pid == std::process::id());
    if let Some(prune) = prune {
        assert!(!prune.can_terminate);
        assert!(prune.protected_reason.is_some());
    }

    // Whatever else is running, nothing with pid 1 is ever offered.
    if let Some(init) = processes.iter().find(|p| p.pid == 1) {
        assert!(!init.can_terminate, "pid 1 was offered as stoppable");
    }
}
