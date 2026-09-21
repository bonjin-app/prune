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
    let asked = monitor.stop_process(sleeper.pid(), StopMode::Ask);

    if cfg!(unix) {
        asked.expect("a process we started should be stoppable");
        assert!(
            wait_for_exit(&mut sleeper, Duration::from_secs(5)),
            "the process was still running after being asked to stop"
        );
    } else {
        // No POSIX signals here, so there is no polite request to send. What matters is that
        // the process is left alone and the reason is stated, rather than forcing it under a
        // label that promised to ask.
        let err = asked.expect_err("asking should not be possible without signals");
        assert!(
            err.to_string().contains("cannot be asked to quit"),
            "unexpected error: {err}"
        );
        assert!(!sleeper.has_exited(), "it should still be running");
        let _ = monitor.stop_process(sleeper.pid(), StopMode::Force);
    }
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
    let err = monitor
        .stop_process(1, StopMode::Force)
        .expect_err("pid 1 must never be stoppable");

    // pid 1 is launchd on macOS and systemd on Linux, and the protection rules name it.
    // Windows has no pid 1 at all, so it is refused earlier and for a different reason — which
    // is still a refusal, and still the right answer.
    let refused = if cfg!(unix) {
        "cannot be stopped"
    } else {
        "no process with id 1"
    };
    assert!(err.to_string().contains(refused), "unexpected error: {err}");
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

#[test]
fn a_recycled_id_does_not_stop_whatever_now_holds_it() {
    // The gap the protection rules leave open: an id that has been reused by an *ordinary*
    // process passes every one of them, so without checking the name the user confirms one
    // program and a different one dies. Asking to stop a real process under the wrong name
    // stands in for that: the answer must be no, and it must still be running afterwards.
    let monitor = monitor();
    let mut sleeper = Sleeper::spawn();

    let err = monitor
        .stop_process_named(sleeper.pid(), StopMode::Force, Some("something-else"))
        .expect_err("a mismatched name must not stop anything");
    assert!(
        err.to_string().contains("not something-else"),
        "the message should say what it found instead: {err}"
    );
    assert!(!sleeper.has_exited(), "it must still be running");

    // Cleaned up through the same call, with the name it really has.
    let name = monitor
        .processes(500)
        .into_iter()
        .find(|p| p.pid == sleeper.pid())
        .map(|p| p.name);
    if let Some(name) = name {
        monitor
            .stop_process_named(sleeper.pid(), StopMode::Force, Some(&name))
            .expect("the right name should be accepted");
        assert!(wait_for_exit(&mut sleeper, Duration::from_secs(5)));
    }
}

#[test]
fn a_caller_with_no_name_to_check_is_still_served() {
    // `prune processes --stop 1234` was never shown a name, so it has none to pass.
    let monitor = monitor();
    let mut sleeper = Sleeper::spawn();

    monitor
        .stop_process_named(sleeper.pid(), StopMode::Force, None)
        .expect("a process we started should be stoppable");
    assert!(wait_for_exit(&mut sleeper, Duration::from_secs(5)));
}
