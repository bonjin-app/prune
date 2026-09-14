use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{Category, RiskLevel};

/// Whether a target is a single file or a directory tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    File,
    Directory,
}

/// A single removable item discovered by a provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupTarget {
    /// Stable id derived from `provider_id` + `path`. Used by the UI to refer to this target
    /// without ever sending the path back to the engine.
    pub id: String,
    pub provider_id: String,
    pub path: String,
    pub kind: TargetKind,
    pub size_bytes: u64,
    pub file_count: u64,
    pub risk: RiskLevel,
    /// Short display name, e.g. `Google Chrome` or `my-project/node_modules`.
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_at: Option<DateTime<Utc>>,
    /// `true` when the item cannot be moved to the trash (e.g. it already *is* trash).
    #[serde(default)]
    pub permanent_only: bool,
}

impl CleanupTarget {
    pub fn path_buf(&self) -> PathBuf {
        PathBuf::from(&self.path)
    }

    /// Deterministic id: FNV-1a 64 of `provider_id\0path`, hex encoded.
    pub fn make_id(provider_id: &str, path: &Path) -> String {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in provider_id
            .bytes()
            .chain(std::iter::once(0))
            .chain(path.to_string_lossy().bytes())
        {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        format!("{hash:016x}")
    }
}

/// Non-fatal problem encountered while scanning (permission denied, vanished file, …).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanIssue {
    pub path: String,
    pub message: String,
}

/// Result of a single provider run.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub provider_id: String,
    pub provider_name: String,
    pub category: Category,
    pub targets: Vec<CleanupTarget>,
    pub total_bytes: u64,
    pub total_files: u64,
    pub duration_ms: u64,
    pub issues: Vec<ScanIssue>,
}

impl ScanResult {
    pub fn recompute_totals(&mut self) {
        self.total_bytes = self.targets.iter().map(|t| t.size_bytes).sum();
        self.total_files = self.targets.iter().map(|t| t.file_count).sum();
    }
}

/// Lifecycle of a scan session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanStatus {
    Running,
    Completed,
    Cancelled,
    Failed,
}

/// Progress event emitted while a scan runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub scan_id: String,
    pub provider_id: String,
    /// Files visited so far by this provider.
    pub scanned_files: u64,
    /// Bytes discovered so far by this provider.
    pub discovered_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_path: Option<String>,
    pub providers_done: usize,
    pub providers_total: usize,
}

/// A complete scan: one result per provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSession {
    pub id: String,
    pub status: ScanStatus,
    pub started_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
    pub results: Vec<ScanResult>,
    pub total_bytes: u64,
    pub total_files: u64,
}

impl ScanSession {
    pub fn find_target(&self, id: &str) -> Option<&CleanupTarget> {
        self.results
            .iter()
            .flat_map(|r| r.targets.iter())
            .find(|t| t.id == id)
    }

    pub fn recompute_totals(&mut self) {
        self.total_bytes = self.results.iter().map(|r| r.total_bytes).sum();
        self.total_files = self.results.iter().map(|r| r.total_files).sum();
    }
}

/// Static description of a provider, for listing in the UI before any scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub category: Category,
    pub description: String,
    pub default_risk: RiskLevel,
    /// Whether this provider has anything to look at on the current machine.
    pub available: bool,
}
