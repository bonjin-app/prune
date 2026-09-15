//! CPU / memory / disk / process information via `sysinfo`.
//!
//! Everything here is read-only except [`SystemMonitor::stop_process`], which is guarded by the
//! rules in [`protection`].

use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

use sysinfo::{Disks, Networks, Pid, ProcessesToUpdate, Signal, System};

use crate::models::{
    CpuStatus, DiskStatus, MemoryStatus, NetworkStatus, Platform, ProcessInfo, StopMode,
    SystemInfo, SystemSnapshot,
};
use crate::{PruneError, Result};

pub mod protection;

use protection::Candidate;

/// Keeps a `sysinfo::System` alive so CPU usage can be measured between calls.
pub struct SystemMonitor {
    inner: Mutex<System>,
    /// Kept alive so each refresh reports what moved since the previous snapshot.
    networks: Mutex<(Networks, Instant)>,
    home: std::path::PathBuf,
}

impl SystemMonitor {
    pub fn new(home: &Path) -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        Self {
            inner: Mutex::new(sys),
            networks: Mutex::new((Networks::new_with_refreshed_list(), Instant::now())),
            home: home.to_path_buf(),
        }
    }

    pub fn info(&self) -> SystemInfo {
        let mut sys = self.inner.lock().unwrap();
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
        let mut sys = self.inner.lock().unwrap();
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        sys.refresh_processes(ProcessesToUpdate::All, true);

        let disks = Disks::new_with_refreshed_list();
        let primary_mount = disks
            .iter()
            .filter(|d| self.home.starts_with(d.mount_point()))
            .max_by_key(|d| d.mount_point().as_os_str().len())
            .map(|d| d.mount_point().to_path_buf());

        let mut disk_list: Vec<DiskStatus> = disks
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
        disk_list.sort_by(|a, b| {
            b.is_primary
                .cmp(&a.is_primary)
                .then(b.total_bytes.cmp(&a.total_bytes))
        });
        disk_list.dedup_by(|a, b| {
            a.total_bytes == b.total_bytes && a.available_bytes == b.available_bytes
        });

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

    /// Throughput since the previous snapshot, plus the totals since boot.
    ///
    /// The first call after start-up has nothing to compare against, so it reports zero rather
    /// than dividing the totals by the uptime and inventing a number.
    fn network(&self) -> NetworkStatus {
        let mut guard = self.networks.lock().unwrap();
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
        let mut sys = self.inner.lock().unwrap();
        sys.refresh_processes(ProcessesToUpdate::All, true);
        let users = sysinfo::Users::new_with_refreshed_list();
        let self_pid = std::process::id();
        let current_user = current_user_name(&sys, &users);

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
        let mut sys = self.inner.lock().unwrap();
        let target = Pid::from_u32(pid);
        sys.refresh_processes(ProcessesToUpdate::Some(&[target]), true);
        let users = sysinfo::Users::new_with_refreshed_list();
        let self_pid = std::process::id();
        let current_user = current_user_name(&sys, &users);

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
            StopMode::Ask => process.kill_with(Signal::Term).unwrap_or(false),
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
fn current_user_name(sys: &System, users: &sysinfo::Users) -> Option<String> {
    sysinfo::get_current_pid()
        .ok()
        .and_then(|pid| sys.process(pid))
        .and_then(|p| p.user_id())
        .and_then(|uid| users.get_user_by_id(uid))
        .map(|u| u.name().to_string())
}
