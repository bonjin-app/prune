//! CPU / memory / disk / process information via `sysinfo`. Read-only.

use std::path::Path;
use std::sync::Mutex;

use sysinfo::{Disks, ProcessesToUpdate, System};

use crate::models::{
    CpuStatus, DiskStatus, MemoryStatus, Platform, ProcessInfo, SystemInfo, SystemSnapshot,
};

/// Keeps a `sysinfo::System` alive so CPU usage can be measured between calls.
pub struct SystemMonitor {
    inner: Mutex<System>,
    home: std::path::PathBuf,
}

impl SystemMonitor {
    pub fn new(home: &Path) -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        Self {
            inner: Mutex::new(sys),
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
            uptime_seconds: System::uptime(),
            process_count: sys.processes().len(),
        }
    }

    /// Top processes by CPU, then memory.
    pub fn processes(&self, limit: usize) -> Vec<ProcessInfo> {
        let mut sys = self.inner.lock().unwrap();
        sys.refresh_processes(ProcessesToUpdate::All, true);
        let users = sysinfo::Users::new_with_refreshed_list();
        let mut list: Vec<ProcessInfo> = sys
            .processes()
            .values()
            .map(|p| ProcessInfo {
                pid: p.pid().as_u32(),
                name: p.name().to_string_lossy().into_owned(),
                cpu_percent: p.cpu_usage(),
                memory_bytes: p.memory(),
                user: p
                    .user_id()
                    .and_then(|uid| users.get_user_by_id(uid))
                    .map(|u| u.name().to_string()),
                parent_pid: p.parent().map(|pp| pp.as_u32()),
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
}
