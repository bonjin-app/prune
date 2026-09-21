//! CPU / memory / disk / process information via `sysinfo`.
//!
//! Everything here is read-only except [`SystemMonitor::stop_process`], which is guarded by the
//! rules in [`protection`].

use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use sysinfo::{
    Disks, Networks, Pid, ProcessRefreshKind, ProcessesToUpdate, Signal, System, UpdateKind, Users,
};

use crate::models::{
    CpuStatus, DiskStatus, MemoryStatus, NetworkStatus, Platform, ProcessInfo, StopMode,
    SystemInfo, SystemSnapshot,
};
use crate::{PruneError, Result};

pub mod protection;

use crate::sync::LockExt;
use protection::Candidate;

/// The shortest gap between two readings that yields a meaningful per-process CPU share.
///
/// A process's CPU usage is work done *between* two samples, so anything that reads it once —
/// a one-shot CLI command, say — has to take two readings this far apart or every process
/// reports zero. The desktop app polls, so its second reading arrives on its own.
pub const CPU_SAMPLE_INTERVAL: Duration = sysinfo::MINIMUM_CPU_UPDATE_INTERVAL;

/// How long a reading of the attached disks is reused.
///
/// Enumerating volumes is by far the most expensive thing a snapshot does — on a machine with
/// a few dozen mounts (APFS snapshots, disk images, anything a container runtime has attached)
/// it costs a third of a second, against a millisecond or two for CPU and memory. The Monitor
/// polls every two seconds, so doing it every time would have Prune sitting near the top of its
/// own process list, which is a poor advertisement for a tool that exists to keep a machine
/// tidy. Capacity does not move on that timescale anyway. What does move it is a cleanup, and
/// [`SystemMonitor::invalidate_disks`] covers that case so freed space shows up at once.
const DISK_TTL: Duration = Duration::from_secs(10);

/// The user list is read to name the owner of a process. People are not added mid-session.
const USERS_TTL: Duration = Duration::from_secs(60);

/// How long a reading of the process table is reused for the process *count*.
///
/// Walking 2,600 processes costs about 60ms even when none of their details are asked for, and
/// the snapshot wants one number out of it. The Monitor view lists processes on its own timer,
/// which refreshes the same table, so most snapshots can simply use what that left behind.
const PROCESS_COUNT_TTL: Duration = Duration::from_secs(4);

/// What a process listing actually reads. Asking for everything else costs 40% more.
fn process_fields() -> ProcessRefreshKind {
    ProcessRefreshKind::nothing()
        .with_cpu()
        .with_memory()
        .with_user(UpdateKind::OnlyIfNotSet)
}

/// Keeps a `sysinfo::System` alive so CPU usage can be measured between calls.
pub struct SystemMonitor {
    inner: Mutex<System>,
    /// Kept alive so each refresh reports what moved since the previous snapshot.
    networks: Mutex<(Networks, Instant)>,
    disks: Mutex<DiskCache>,
    users: Mutex<(Users, Instant)>,
    /// When the process table was last walked, by either entry point.
    processes_read_at: Mutex<Option<Instant>>,
    home: std::path::PathBuf,
}

struct DiskCache {
    disks: Disks,
    status: Vec<DiskStatus>,
    /// `None` until the first reading, and again whenever something invalidates it.
    refreshed_at: Option<Instant>,
}

impl SystemMonitor {
    pub fn new(home: &Path) -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        Self {
            inner: Mutex::new(sys),
            networks: Mutex::new((Networks::new_with_refreshed_list(), Instant::now())),
            disks: Mutex::new(DiskCache {
                disks: Disks::new(),
                status: Vec::new(),
                refreshed_at: None,
            }),
            users: Mutex::new((Users::new(), Instant::now() - USERS_TTL)),
            processes_read_at: Mutex::new(None),
            home: home.to_path_buf(),
        }
    }

    /// Makes the next snapshot read the disks again rather than reuse the cached reading.
    ///
    /// Call this after removing anything: "you just freed 12 GB" is exactly the moment a stale
    /// free-space figure is most obvious.
    pub fn invalidate_disks(&self) {
        self.disks.lock_recover().refreshed_at = None;
    }

    pub fn info(&self) -> SystemInfo {
        let mut sys = self.inner.lock_recover();
        sys.refresh_cpu_list(sysinfo::CpuRefreshKind::nothing());
        sys.refresh_memory();
        let cpu_brand = sys
            .cpus()
            .first()
            .map(|c| c.brand().trim().to_string())
            .unwrap_or_default();
        SystemInfo {
            platform: Platform::current(),
            os_name: System::name().unwrap_or_else(|| "Unknown".into()),
            os_version: System::os_version().unwrap_or_default(),
            kernel_version: System::kernel_version().unwrap_or_default(),
            hostname: System::host_name().unwrap_or_default(),
            arch: System::cpu_arch(),
            cpu_brand,
            physical_cores: System::physical_core_count(),
            logical_cores: sys.cpus().len(),
            total_memory_bytes: sys.total_memory(),
            home_dir: self.home.to_string_lossy().into_owned(),
        }
    }

    pub fn snapshot(&self) -> SystemSnapshot {
        let mut sys = self.inner.lock_recover();
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        // Only the count is read here, so there is nothing to gain from asking for each
        // process's details as well — nor from walking the table at all if the process list was
        // read a moment ago, which is exactly what happens while the Monitor view is open.
        let mut read_at = self.processes_read_at.lock_recover();
        if !read_at.is_some_and(|t| t.elapsed() < PROCESS_COUNT_TTL) {
            sys.refresh_processes_specifics(
                ProcessesToUpdate::All,
                true,
                ProcessRefreshKind::nothing(),
            );
            *read_at = Some(Instant::now());
        }
        drop(read_at);

        let disk_list = self.disks();

        SystemSnapshot {
            cpu: CpuStatus {
                usage_percent: sys.global_cpu_usage(),
                per_core_percent: sys.cpus().iter().map(|c| c.cpu_usage()).collect(),
            },
            memory: MemoryStatus {
                total_bytes: sys.total_memory(),
                used_bytes: sys.used_memory(),
                available_bytes: sys.available_memory(),
                swap_total_bytes: sys.total_swap(),
                swap_used_bytes: sys.used_swap(),
            },
            disks: disk_list,
            network: self.network(),
            uptime_seconds: System::uptime(),
            process_count: sys.processes().len(),
        }
    }

    /// The attached volumes, re-read at most once per [`DISK_TTL`].
    fn disks(&self) -> Vec<DiskStatus> {
        let mut cache = self.disks.lock_recover();
        let fresh = cache.refreshed_at.is_some_and(|at| at.elapsed() < DISK_TTL);
        if fresh {
            return cache.status.clone();
        }

        cache.disks.refresh(true);
        let home = &self.home;
        let primary_mount = cache
            .disks
            .iter()
            .filter(|d| home.starts_with(d.mount_point()))
            .max_by_key(|d| d.mount_point().as_os_str().len())
            .map(|d| d.mount_point().to_path_buf());

        let mut list: Vec<DiskStatus> = cache
            .disks
            .iter()
            .filter(|d| d.total_space() > 0)
            .map(|d| DiskStatus {
                name: d.name().to_string_lossy().into_owned(),
                mount_point: d.mount_point().to_string_lossy().into_owned(),
                file_system: d.file_system().to_string_lossy().into_owned(),
                total_bytes: d.total_space(),
                available_bytes: d.available_space(),
                is_removable: d.is_removable(),
                is_primary: primary_mount.as_deref() == Some(d.mount_point()),
            })
            .collect();
        // Primary first, then by size. Hide APFS helper volumes that mirror the primary.
        list.sort_by(|a, b| {
            b.is_primary
                .cmp(&a.is_primary)
                .then(b.total_bytes.cmp(&a.total_bytes))
        });
        list.dedup_by(|a, b| {
            a.total_bytes == b.total_bytes && a.available_bytes == b.available_bytes
        });

        cache.status = list.clone();
        cache.refreshed_at = Some(Instant::now());
        list
    }

    /// The user list, re-read at most once per [`USERS_TTL`].
    ///
    /// Returns the guard rather than a copy because `Users` is not cloneable. Callers already
    /// hold `inner`, and nothing takes these two locks in the other order.
    fn users(&self) -> std::sync::MutexGuard<'_, (Users, Instant)> {
        let mut guard = self.users.lock_recover();
        let (users, refreshed_at) = &mut *guard;
        if refreshed_at.elapsed() >= USERS_TTL {
            users.refresh();
            *refreshed_at = Instant::now();
        }
        guard
    }

    /// Throughput since the previous snapshot, plus the totals since boot.
    ///
    /// The first call after start-up has nothing to compare against, so it reports zero rather
    /// than dividing the totals by the uptime and inventing a number.
    fn network(&self) -> NetworkStatus {
        let mut guard = self.networks.lock_recover();
        let (networks, last) = &mut *guard;
        networks.refresh(true);
        let elapsed = last.elapsed().as_secs_f64();
        *last = Instant::now();

        let mut received = 0u64;
        let mut transmitted = 0u64;
        let mut total_received = 0u64;
        let mut total_transmitted = 0u64;
        for data in networks.values() {
            received += data.received();
            transmitted += data.transmitted();
            total_received += data.total_received();
            total_transmitted += data.total_transmitted();
        }
        let rate = |bytes: u64| {
            if elapsed >= 0.2 {
                (bytes as f64 / elapsed) as u64
            } else {
                0
            }
        };
        NetworkStatus {
            down_bytes_per_sec: rate(received),
            up_bytes_per_sec: rate(transmitted),
            total_received_bytes: total_received,
            total_transmitted_bytes: total_transmitted,
        }
    }

    /// Top processes by CPU, then memory.
    pub fn processes(&self, limit: usize) -> Vec<ProcessInfo> {
        let mut sys = self.inner.lock_recover();
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, process_fields());
        *self.processes_read_at.lock_recover() = Some(Instant::now());
        let users = self.users();
        let users = &users.0;
        let self_pid = std::process::id();
        let current_user = current_user_name(&sys, users);

        let mut list: Vec<ProcessInfo> = sys
            .processes()
            .values()
            .map(|p| {
                let pid = p.pid().as_u32();
                let name = p.name().to_string_lossy().into_owned();
                let user = p
                    .user_id()
                    .and_then(|uid| users.get_user_by_id(uid))
                    .map(|u| u.name().to_string());
                let protection = protection::classify(Candidate {
                    pid,
                    name: &name,
                    user: user.as_deref(),
                    current_user: current_user.as_deref(),
                    self_pid,
                });
                ProcessInfo {
                    pid,
                    name,
                    cpu_percent: p.cpu_usage(),
                    memory_bytes: p.memory(),
                    user,
                    parent_pid: p.parent().map(|pp| pp.as_u32()),
                    can_terminate: protection.is_none(),
                    protected_reason: protection.map(|r| r.message().to_string()),
                }
            })
            .collect();
        list.sort_by(|a, b| {
            b.cpu_percent
                .partial_cmp(&a.cpu_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.memory_bytes.cmp(&a.memory_bytes))
        });
        list.truncate(limit);
        list
    }

    /// Stops a process, after checking it is one Prune is willing to stop.
    ///
    /// The protection rules are applied here, not trusted from the caller: the UI sends a
    /// process id, and by the time it arrives that id may belong to something else entirely.
    pub fn stop_process(&self, pid: u32, mode: StopMode) -> Result<()> {
        let mut sys = self.inner.lock_recover();
        let target = Pid::from_u32(pid);
        sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[target]), true, process_fields());
        let users = self.users();
        let users = &users.0;
        let self_pid = std::process::id();
        let current_user = current_user_name(&sys, users);

        let process = sys
            .process(target)
            .ok_or_else(|| PruneError::Other(format!("no process with id {pid}")))?;
        let name = process.name().to_string_lossy().into_owned();
        let user = process
            .user_id()
            .and_then(|uid| users.get_user_by_id(uid))
            .map(|u| u.name().to_string());

        if let Some(reason) = protection::classify(Candidate {
            pid,
            name: &name,
            user: user.as_deref(),
            current_user: current_user.as_deref(),
            self_pid,
        }) {
            return Err(PruneError::Other(format!(
                "{name} cannot be stopped: {}",
                reason.message()
            )));
        }

        let signalled = match mode {
            StopMode::Ask => match process.kill_with(Signal::Term) {
                Some(sent) => sent,
                // Windows has no POSIX signals, so there is no polite request to send. Saying
                // so is better than the alternatives: reporting a refusal the system never
                // made, or quietly forcing instead — which would take unsaved work with it
                // under a label that promised to ask first.
                None => {
                    return Err(PruneError::Other(format!(
                        "{name} cannot be asked to quit on this system; only forcing it to stop                          is available, which loses anything unsaved"
                    )))
                }
            },
            StopMode::Force => process.kill(),
        };
        if signalled {
            tracing::info!(pid, name, ?mode, "asked a process to stop");
            Ok(())
        } else {
            Err(PruneError::Other(format!(
                "the system refused to stop {name}"
            )))
        }
    }
}

/// The user Prune is running as, read from its own process so it matches what the process list
/// reports for everyone else.
fn current_user_name(sys: &System, users: &Users) -> Option<String> {
    sysinfo::get_current_pid()
        .ok()
        .and_then(|pid| sys.process(pid))
        .and_then(|p| p.user_id())
        .and_then(|uid| users.get_user_by_id(uid))
        .map(|u| u.name().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// These read the machine the tests run on, which is fine: everything here is read-only and
    /// touches no path. What they assert is the caching, not the values.
    fn monitor() -> SystemMonitor {
        SystemMonitor::new(Path::new("/"))
    }

    #[test]
    fn a_repeated_snapshot_reuses_the_disk_reading() {
        let m = monitor();
        let first = m.snapshot();
        let second = m.snapshot();
        // Same volumes, same figures: the second call did not go back to the filesystem.
        assert_eq!(first.disks.len(), second.disks.len());
        assert!(
            m.disks.lock_recover().refreshed_at.is_some(),
            "the first snapshot should have populated the cache"
        );
    }

    #[test]
    fn invalidating_forces_the_next_snapshot_to_look_again() {
        let m = monitor();
        let _ = m.snapshot();
        assert!(m.disks.lock_recover().refreshed_at.is_some());

        m.invalidate_disks();
        assert!(
            m.disks.lock_recover().refreshed_at.is_none(),
            "a cleanup must not leave a stale free-space figure behind"
        );

        let after = m.snapshot();
        assert!(!after.disks.is_empty());
        assert!(m.disks.lock_recover().refreshed_at.is_some());
    }

    #[test]
    fn listing_processes_satisfies_the_next_snapshots_count() {
        let m = monitor();
        let listed = m.processes(5);
        assert!(!listed.is_empty());
        let read_at = *m.processes_read_at.lock_recover();
        assert!(read_at.is_some(), "the listing should record when it read");

        let snapshot = m.snapshot();
        assert!(snapshot.process_count > 0);
        assert_eq!(
            *m.processes_read_at.lock_recover(),
            read_at,
            "the snapshot should have reused the listing's walk, not repeated it"
        );
    }

    #[test]
    fn a_first_snapshot_still_counts_processes() {
        let m = monitor();
        assert!(m.snapshot().process_count > 0);
    }

    #[test]
    fn the_process_list_reports_what_the_ui_shows() {
        let m = monitor();
        let listed = m.processes(5);
        assert!(listed.len() <= 5);
        // The fields the process refresh was narrowed to must all still arrive.
        assert!(listed.iter().all(|p| !p.name.is_empty()));
        assert!(listed.iter().any(|p| p.memory_bytes > 0));
        // Prune refuses to stop itself, so at least one protected entry is always reachable.
        assert!(m.processes(500).iter().any(|p| !p.can_terminate));
    }
}
