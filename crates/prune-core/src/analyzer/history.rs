//! The last finished analysis of each folder, kept so the Storage view has something to show
//! before a new one finishes. On a home folder that can take minutes.
//!
//! What is kept is deliberately small: the totals and the largest folders directly inside the
//! one analysed. Not the tree and not file names, so the record stays a few kilobytes. It is a
//! record of what was, labelled with when. Nothing can be removed from it. Removal always works
//! from a fresh analysis, through the usual preview, so a stale record can never be the reason
//! something goes.
//!
//! Only analyses that ran to the end are kept. A cancelled one would show a partial total as if
//! it were the whole folder.

use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{DiskAnalysis, DiskScanStatus};
use crate::{PruneError, Result};

const FILE_NAME: &str = "disk-history.json";
/// Folders remembered. Older ones drop off first.
const MAX_ROOTS: usize = 8;
/// Largest subfolders kept per analysis.
const MAX_FOLDERS: usize = 12;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedAnalysis {
    pub root: String,
    pub finished_at: DateTime<Utc>,
    pub total_bytes: u64,
    pub file_count: u64,
    pub dir_count: u64,
    pub duration_ms: u64,
    /// Paths that could not be read, so the total may be low.
    pub issue_count: usize,
    /// The largest folders directly inside `root`, largest first.
    pub folders: Vec<SavedFolder>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedFolder {
    pub name: String,
    pub size_bytes: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct HistoryFile {
    analyses: Vec<SavedAnalysis>,
}

/// The remembered analyses, most recent first. A missing or unreadable file is no history:
/// this is a convenience, and never a reason for the Storage view to fail.
pub fn list(dir: &Path) -> Vec<SavedAnalysis> {
    std::fs::read_to_string(dir.join(FILE_NAME))
        .ok()
        .and_then(|text| serde_json::from_str::<HistoryFile>(&text).ok())
        .map(|file| file.analyses)
        .unwrap_or_default()
}

/// Remembers `analysis` if it ran to the end, replacing any earlier one of the same folder.
/// Returns whether it was kept.
pub fn record(dir: &Path, analysis: &DiskAnalysis) -> Result<bool> {
    let Some(saved) = summarise(analysis) else {
        return Ok(false);
    };
    let mut analyses = list(dir);
    analyses.retain(|a| a.root != saved.root);
    analyses.insert(0, saved);
    analyses.truncate(MAX_ROOTS);
    write(dir, &HistoryFile { analyses })?;
    Ok(true)
}

fn summarise(analysis: &DiskAnalysis) -> Option<SavedAnalysis> {
    if analysis.status != DiskScanStatus::Completed {
        return None;
    }
    let view = analysis.node(None)?;
    let summary = analysis.summary();
    Some(SavedAnalysis {
        root: summary.root,
        finished_at: Utc::now(),
        total_bytes: summary.total_bytes,
        file_count: summary.file_count,
        dir_count: summary.dir_count,
        duration_ms: summary.duration_ms,
        issue_count: summary.issue_count,
        folders: view
            .children
            .into_iter()
            .filter(|c| c.size_bytes > 0)
            .take(MAX_FOLDERS)
            .map(|c| SavedFolder {
                name: c.name,
                size_bytes: c.size_bytes,
            })
            .collect(),
    })
}

/// Written to a temporary file and renamed over the old one, so a crash leaves either history
/// whole, never half of one.
fn write(dir: &Path, file: &HistoryFile) -> Result<()> {
    std::fs::create_dir_all(dir).map_err(|e| PruneError::io(dir, e))?;
    let path = dir.join(FILE_NAME);
    let temp = path.with_extension("json.writing");
    std::fs::write(&temp, serde_json::to_string_pretty(file)?)
        .map_err(|e| PruneError::io(&temp, e))?;
    std::fs::rename(&temp, &path).map_err(|e| {
        let _ = std::fs::remove_file(&temp);
        PruneError::io(&path, e)
    })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use super::*;
    use crate::analyzer::analyze;
    use crate::platform::ProtectedPaths;
    use crate::safety::SafetyPolicy;

    fn tree(root: &Path) {
        for (path, size) in [("big/a.bin", 5_000), ("small/b.txt", 10), ("loose.txt", 3)] {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, vec![0u8; size]).unwrap();
        }
    }

    fn run(root: &Path, cancelled: bool) -> DiskAnalysis {
        let policy = SafetyPolicy::from_protected(ProtectedPaths {
            allowed_roots: vec![root.to_path_buf()],
            ..Default::default()
        });
        let cancel = AtomicBool::new(cancelled);
        analyze("t", root, &policy, &cancel, &|_| {})
    }

    #[test]
    fn keeps_totals_and_largest_folders_of_a_finished_analysis() {
        let work = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        tree(work.path());
        assert!(record(data.path(), &run(work.path(), false)).unwrap());

        let saved = list(data.path());
        assert_eq!(saved.len(), 1);
        assert!(saved[0].total_bytes >= 5_013);
        let names: Vec<&str> = saved[0].folders.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["big", "small"]);
    }

    #[test]
    fn a_cancelled_analysis_is_not_kept() {
        let work = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        tree(work.path());
        assert!(!record(data.path(), &run(work.path(), true)).unwrap());
        assert!(list(data.path()).is_empty());
    }

    #[test]
    fn a_new_analysis_replaces_the_old_one_and_the_newest_comes_first() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        tree(a.path());
        tree(b.path());
        record(data.path(), &run(a.path(), false)).unwrap();
        record(data.path(), &run(b.path(), false)).unwrap();
        record(data.path(), &run(a.path(), false)).unwrap();
        let roots: Vec<String> = list(data.path()).into_iter().map(|s| s.root).collect();
        assert_eq!(roots.len(), 2);
        assert!(Path::new(&roots[0]).ends_with(a.path().file_name().unwrap()));
    }

    #[test]
    fn an_unreadable_history_is_no_history_and_is_replaced_on_the_next_record() {
        let work = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        std::fs::write(data.path().join(FILE_NAME), b"{ not json").unwrap();
        assert!(list(data.path()).is_empty());
        tree(work.path());
        record(data.path(), &run(work.path(), false)).unwrap();
        assert_eq!(list(data.path()).len(), 1);
    }
}
