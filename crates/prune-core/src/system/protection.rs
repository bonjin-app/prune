//! Deciding which processes Prune refuses to stop.
//!
//! Stopping the wrong process logs the user out, freezes the desktop or takes down the
//! machine, and unlike a deleted cache there is no trash to recover from. The rules are
//! therefore conservative and live in one pure function so they can be tested exhaustively.

/// Why a process cannot be stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protection {
    /// `init`/`launchd` and the kernel. Stopping these takes the machine down.
    SystemCritical,
    /// Owned by someone else, so stopping it would need administrator rights anyway.
    OtherUser,
    /// Prune itself. Stopping it mid-cleanup is never what the user meant.
    Ourselves,
}

impl Protection {
    pub fn message(self) -> &'static str {
        match self {
            Protection::SystemCritical => {
                "part of the operating system; stopping it would take the machine down"
            }
            Protection::OtherUser => "owned by another user, so it needs administrator rights",
            Protection::Ourselves => "this is Prune",
        }
    }
}

/// Processes that keep the session or the machine alive. Matched case-insensitively against
/// the process name, which is what both platforms report.
const CRITICAL: &[&str] = &[
    // macOS
    "kernel_task",
    "launchd",
    "logind",
    "loginwindow",
    "windowserver",
    "systemuiserver",
    "coreaudiod",
    "opendirectoryd",
    "securityd",
    "distnoted",
    "notifyd",
    "configd",
    "diskarbitrationd",
    "mds",
    "mds_stores",
    "powerd",
    "watchdogd",
    "finder",
    "dock",
    // Windows
    "system",
    "system idle process",
    "smss.exe",
    "csrss.exe",
    "wininit.exe",
    "winlogon.exe",
    "services.exe",
    "lsass.exe",
    "lsaiso.exe",
    "svchost.exe",
    "dwm.exe",
    "explorer.exe",
    "fontdrvhost.exe",
    "sihost.exe",
    "ctfmon.exe",
    // Linux, for completeness
    "systemd",
    "init",
    "dbus-daemon",
];

/// Everything needed to judge one process.
#[derive(Debug, Clone, Copy)]
pub struct Candidate<'a> {
    pub pid: u32,
    pub name: &'a str,
    /// The owning user, when known.
    pub user: Option<&'a str>,
    /// The user Prune is running as.
    pub current_user: Option<&'a str>,
    /// Prune's own process id.
    pub self_pid: u32,
}

/// `None` when the process may be stopped.
pub fn classify(candidate: Candidate<'_>) -> Option<Protection> {
    if candidate.pid == candidate.self_pid {
        return Some(Protection::Ourselves);
    }
    // pid 0 and 1 are the kernel and the init process on every platform Prune targets.
    if candidate.pid <= 1 {
        return Some(Protection::SystemCritical);
    }
    let name = candidate.name.trim().to_ascii_lowercase();
    if CRITICAL.contains(&name.as_str()) {
        return Some(Protection::SystemCritical);
    }
    // Anything owned by someone else needs privileges Prune does not ask for.
    match (candidate.user, candidate.current_user) {
        (Some(owner), Some(me)) if owner != me => Some(Protection::OtherUser),
        // An unknown owner usually means a system process the user cannot read.
        (None, Some(_)) => Some(Protection::OtherUser),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate<'a>(pid: u32, name: &'a str, user: Option<&'a str>) -> Candidate<'a> {
        Candidate {
            pid,
            name,
            user,
            current_user: Some("dev"),
            self_pid: 4242,
        }
    }

    #[test]
    fn allows_an_ordinary_process_of_this_user() {
        assert_eq!(classify(candidate(900, "node", Some("dev"))), None);
        assert_eq!(
            classify(candidate(901, "Google Chrome Helper", Some("dev"))),
            None
        );
    }

    #[test]
    fn refuses_the_kernel_and_init() {
        assert_eq!(
            classify(candidate(0, "kernel_task", Some("dev"))),
            Some(Protection::SystemCritical)
        );
        assert_eq!(
            classify(candidate(1, "anything", Some("dev"))),
            Some(Protection::SystemCritical)
        );
    }

    #[test]
    fn refuses_processes_that_hold_the_session_together() {
        for name in [
            "WindowServer",
            "loginwindow",
            "launchd",
            "Finder",
            "lsass.exe",
            "csrss.exe",
            "explorer.exe",
            "systemd",
        ] {
            assert_eq!(
                classify(candidate(500, name, Some("dev"))),
                Some(Protection::SystemCritical),
                "{name} must be protected"
            );
        }
    }

    #[test]
    fn matching_ignores_case_and_surrounding_space() {
        assert_eq!(
            classify(candidate(500, "  WINDOWSERVER  ", Some("dev"))),
            Some(Protection::SystemCritical)
        );
    }

    #[test]
    fn refuses_other_users_and_unknown_owners() {
        assert_eq!(
            classify(candidate(900, "postgres", Some("root"))),
            Some(Protection::OtherUser)
        );
        assert_eq!(
            classify(candidate(900, "mystery", None)),
            Some(Protection::OtherUser)
        );
    }

    #[test]
    fn refuses_prune_itself() {
        assert_eq!(
            classify(candidate(4242, "prune", Some("dev"))),
            Some(Protection::Ourselves)
        );
    }

    #[test]
    fn every_reason_explains_itself() {
        for reason in [
            Protection::SystemCritical,
            Protection::OtherUser,
            Protection::Ourselves,
        ] {
            assert!(!reason.message().is_empty());
        }
    }
}
