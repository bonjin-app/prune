use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{CleanupTarget, DeleteMode};

/// A target that was requested but refused by the safety layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockedTarget {
    pub target_id: String,
    pub path: String,
    pub reason: String,
}

/// Dry run: exactly what *would* be removed. Nothing is touched while building a plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupPlan {
    pub id: String,
    pub scan_id: String,
    pub mode: DeleteMode,
    pub created_at: DateTime<Utc>,
    pub targets: Vec<CleanupTarget>,
    pub blocked: Vec<BlockedTarget>,
    pub total_bytes: u64,
    pub file_count: u64,
    pub directory_count: u64,
}

/// Progress event emitted while a plan executes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupProgress {
    pub plan_id: String,
    pub done: usize,
    pub total: usize,
    pub current_path: String,
    pub removed_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FailedTarget {
    pub target_id: String,
    pub path: String,
    pub error: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupStatus {
    Success,
    Partial,
    Failed,
}

/// Outcome of executing a [`CleanupPlan`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupResult {
    pub operation_id: String,
    pub plan_id: String,
    pub mode: DeleteMode,
    pub status: CleanupStatus,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub removed_targets: usize,
    pub removed_files: u64,
    pub removed_bytes: u64,
    pub failed: Vec<FailedTarget>,
}

/// One line in the local operation log.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationRecord {
    pub id: String,
    pub at: DateTime<Utc>,
    pub title: String,
    pub mode: DeleteMode,
    pub status: CleanupStatus,
    pub removed_targets: usize,
    pub removed_files: u64,
    pub removed_bytes: u64,
    pub failed_count: usize,
    /// Provider ids involved, for display ("Clean Developer Cache").
    pub providers: Vec<String>,
}
