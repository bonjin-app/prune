use std::path::PathBuf;

use serde::Serialize;
use thiserror::Error;

/// All errors produced by prune-core.
///
/// The `code` is stable and machine-readable so the UI can branch on it; the message is
/// human-readable and safe to display.
#[derive(Debug, Error)]
pub enum PruneError {
    #[error("path is not allowed: {reason} ({path})")]
    Safety { path: PathBuf, reason: String },

    #[error("unknown scan session: {0}")]
    UnknownScan(String),

    #[error("unknown cleanup plan: {0}")]
    UnknownPlan(String),

    #[error("unknown target: {0}")]
    UnknownTarget(String),

    #[error("scan was cancelled")]
    Cancelled,

    #[error("feature not available yet on this platform: {0}")]
    NotImplemented(&'static str),

    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("trash error at {path}: {message}")]
    Trash { path: PathBuf, message: String },

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}

impl PruneError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub fn safety(path: impl Into<PathBuf>, reason: impl Into<String>) -> Self {
        Self::Safety {
            path: path.into(),
            reason: reason.into(),
        }
    }

    /// Stable, machine-readable error code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Safety { .. } => "safety_violation",
            Self::UnknownScan(_) => "unknown_scan",
            Self::UnknownPlan(_) => "unknown_plan",
            Self::UnknownTarget(_) => "unknown_target",
            Self::Cancelled => "cancelled",
            Self::NotImplemented(_) => "not_implemented",
            Self::Io { .. } => "io",
            Self::Trash { .. } => "trash",
            Self::Serde(_) => "serde",
            Self::Other(_) => "other",
        }
    }
}

/// Serializable projection of [`PruneError`] for IPC boundaries.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorInfo {
    pub code: &'static str,
    pub message: String,
}

impl From<&PruneError> for ErrorInfo {
    fn from(err: &PruneError) -> Self {
        Self {
            code: err.code(),
            message: err.to_string(),
        }
    }
}

pub type Result<T> = std::result::Result<T, PruneError>;
