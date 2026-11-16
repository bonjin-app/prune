use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Macos,
    Windows,
    Linux,
    Unknown,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Platform::Macos
        } else if cfg!(target_os = "windows") {
            Platform::Windows
        } else if cfg!(target_os = "linux") {
            Platform::Linux
        } else {
            Platform::Unknown
        }
    }
}

/// Static facts about the machine. Cheap to compute once.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub platform: Platform,
    pub os_name: String,
    pub os_version: String,
    pub kernel_version: String,
    pub hostname: String,
    pub arch: String,
    pub cpu_brand: String,
    pub physical_cores: Option<usize>,
    pub logical_cores: usize,
    pub total_memory_bytes: u64,
    pub home_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuStatus {
    /// 0.0 – 100.0
    pub usage_percent: f32,
    pub per_core_percent: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryStatus {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_used_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskStatus {
    pub name: String,
    pub mount_point: String,
    pub file_system: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub is_removable: bool,
    /// The volume that holds the user's home directory.
    pub is_primary: bool,
}

/// Network throughput, measured between snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStatus {
    pub down_bytes_per_sec: u64,
    pub up_bytes_per_sec: u64,
    /// Totals since the machine booted, across all interfaces.
    pub total_received_bytes: u64,
    pub total_transmitted_bytes: u64,
}

/// Live snapshot. Call repeatedly; CPU usage and network rates are measured between calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemSnapshot {
    pub cpu: CpuStatus,
    pub memory: MemoryStatus,
    pub disks: Vec<DiskStatus>,
    pub network: NetworkStatus,
    pub uptime_seconds: u64,
    pub process_count: usize,
    /// `None` on a machine without a battery, or where it cannot be read.
    pub battery: Option<BatteryStatus>,
    pub temperatures: Temperatures,
    /// Graphics processors, as the OS reports them. Empty where none can be read.
    #[serde(default)]
    pub gpus: Vec<GpuStatus>,
}

/// A graphics processor. Every field the OS may withhold is optional and shown as unknown.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuStatus {
    /// The model, where the OS names it (`Apple M3 Pro`).
    pub name: Option<String>,
    /// How busy it is, 0–100: the busiest of its engines.
    pub usage_percent: Option<f32>,
    /// Memory it is using. On Apple silicon this is shared with the processor.
    pub memory_used_bytes: Option<u64>,
    /// Memory of its own, where it has some (a discrete card).
    pub memory_total_bytes: Option<u64>,
    pub core_count: Option<u32>,
}

/// The battery as the operating system reports it. Every field the OS may withhold is optional,
/// and shown as unknown rather than guessed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatteryStatus {
    /// Charge, 0–100.
    pub percent: f32,
    /// On mains power. Plugged in is not the same as charging: a full or held battery is not.
    pub plugged_in: bool,
    pub charging: bool,
    /// Minutes until empty when discharging, until full when charging. `None` while the OS is
    /// still estimating, or when neither applies.
    pub minutes_remaining: Option<u32>,
    pub cycle_count: Option<u32>,
    /// The cycle count the battery is designed for, where the OS says.
    pub design_cycle_count: Option<u32>,
    /// What a full charge holds now, as a share of what it held new, 0–100.
    pub capacity_percent: Option<f32>,
}

/// Readings from the machine's own temperature sensors, in °C. Each is `None` where there is no
/// such sensor or it cannot be read without elevated rights (most Windows machines).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Temperatures {
    /// The hottest processor sensor.
    pub cpu_celsius: Option<f32>,
    pub battery_celsius: Option<f32>,
    /// The hottest internal storage sensor.
    pub storage_celsius: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_pid: Option<u32>,
    /// Whether Prune will let this process be stopped.
    pub can_terminate: bool,
    /// Why not, when it will not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protected_reason: Option<String>,
}

/// How to stop a process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopMode {
    /// Ask it to exit and let it save its work. What "Quit" means everywhere else.
    Ask,
    /// Stop it immediately. Unsaved work is lost.
    Force,
}
