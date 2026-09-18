//! Local, append-only operation log (JSON lines). Never leaves the machine.

use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::models::OperationRecord;
use crate::sync::LockExt;
use crate::{PruneError, Result};

/// When the log is shortened, and to how much.
///
/// Generous enough that nobody loses history they would actually look at — a machine cleaned
/// every week reaches a thousand entries after twenty years — and small enough that reading it
/// stays instant.
const MAX_BYTES: u64 = 512 * 1024;
const KEEP_RECORDS: usize = 1_000;

pub struct OperationLog {
    dir: PathBuf,
    path: PathBuf,
    lock: Mutex<()>,
}

impl OperationLog {
    /// The log lives at `<dir>/operations.jsonl`.
    ///
    /// Opening never fails. The directory is created when something is first written, so an
    /// unwritable data directory costs the user their history rather than the application:
    /// refusing to start over a log would leave a bundled app with no console simply not
    /// opening, and nothing to explain why.
    pub fn open(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
            path: dir.join("operations.jsonl"),
            lock: Mutex::new(()),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&self, record: &OperationRecord) -> Result<()> {
        let _guard = self.lock.lock_recover();
        std::fs::create_dir_all(&self.dir).map_err(|e| PruneError::io(&self.dir, e))?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| PruneError::io(&self.path, e))?;
        let mut line = serde_json::to_string(record)?;
        line.push('\n');
        file.write_all(line.as_bytes())
            .map_err(|e| PruneError::io(&self.path, e))?;
        drop(file);

        // Nothing ever removed entries, so a machine cleaned weekly for years would grow a log
        // that `list` reads from end to end every time the history is opened.
        if self.size().is_some_and(|size| size > MAX_BYTES) {
            if let Err(e) = self.compact() {
                tracing::warn!(error = %e, "could not shorten the operation log");
            }
        }
        Ok(())
    }

    fn size(&self) -> Option<u64> {
        std::fs::metadata(&self.path).ok().map(|m| m.len())
    }

    /// Rewrites the log with only the most recent entries.
    ///
    /// Written to a temporary file and renamed, so an interrupted compaction leaves the
    /// previous log intact rather than a truncated one.
    fn compact(&self) -> Result<()> {
        let text =
            std::fs::read_to_string(&self.path).map_err(|e| PruneError::io(&self.path, e))?;
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        if lines.len() <= KEEP_RECORDS {
            return Ok(());
        }
        let kept = lines[lines.len() - KEEP_RECORDS..].join("\n");
        let temp = self.path.with_extension("jsonl.writing");
        std::fs::write(&temp, format!("{kept}\n")).map_err(|e| PruneError::io(&temp, e))?;
        std::fs::rename(&temp, &self.path).map_err(|e| {
            let _ = std::fs::remove_file(&temp);
            PruneError::io(&self.path, e)
        })?;
        tracing::info!(
            kept = KEEP_RECORDS,
            dropped = lines.len() - KEEP_RECORDS,
            "shortened the operation log"
        );
        Ok(())
    }

    /// Most recent first. Corrupt lines are skipped.
    pub fn list(&self, limit: usize) -> Result<Vec<OperationRecord>> {
        let _guard = self.lock.lock_recover();
        // A log that cannot be opened has no history to show: nothing could have been written
        // to it either. Reporting an error here would put a failure in front of the user on
        // every visit to a screen that would have been empty anyway.
        let file = match std::fs::File::open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => {
                tracing::warn!(path = %self.path.display(), error = %e, "cannot read the operation log");
                return Ok(vec![]);
            }
        };
        let mut records: Vec<OperationRecord> = BufReader::new(file)
            .lines()
            .map_while(|l| l.ok())
            .filter_map(|l| serde_json::from_str(&l).ok())
            .collect();
        records.reverse();
        records.truncate(limit);
        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CleanupStatus, DeleteMode};
    use chrono::Utc;

    #[test]
    fn append_and_list_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let log = OperationLog::open(dir.path());
        for i in 0..3 {
            log.append(&OperationRecord {
                id: format!("op{i}"),
                at: Utc::now(),
                title: "Clean".into(),
                mode: DeleteMode::Trash,
                status: CleanupStatus::Success,
                removed_targets: 1,
                removed_files: 1,
                removed_bytes: 1,
                failed_count: 0,
                providers: vec![],
            })
            .unwrap();
        }
        let list = log.list(2).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, "op2");
    }
}

#[cfg(test)]
mod resilience_tests {
    use super::*;

    #[test]
    fn opening_never_fails_even_where_nothing_can_be_written() {
        // A path that cannot become a directory, which is what an unwritable or full data
        // directory looks like. The application must still start.
        let dir = tempfile::tempdir().unwrap();
        let blocked = dir.path().join("a-file");
        std::fs::write(&blocked, b"x").unwrap();

        let log = OperationLog::open(&blocked.join("log"));

        // Reading is empty rather than an error...
        assert!(log.list(10).unwrap().is_empty());
        // ...and writing reports the problem, which the caller carries in its result.
        assert!(log
            .append(&OperationRecord {
                id: "op".into(),
                at: chrono::Utc::now(),
                title: "Clean".into(),
                mode: crate::models::DeleteMode::Trash,
                status: crate::models::CleanupStatus::Success,
                removed_targets: 1,
                removed_files: 1,
                removed_bytes: 1,
                failed_count: 0,
                providers: vec![],
            })
            .is_err());
    }

    #[test]
    fn the_directory_appears_when_something_is_first_written() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("not/created/yet");
        let log = OperationLog::open(&nested);
        assert!(!nested.exists());

        log.append(&OperationRecord {
            id: "op".into(),
            at: chrono::Utc::now(),
            title: "Clean".into(),
            mode: crate::models::DeleteMode::Trash,
            status: crate::models::CleanupStatus::Success,
            removed_targets: 1,
            removed_files: 1,
            removed_bytes: 1,
            failed_count: 0,
            providers: vec![],
        })
        .unwrap();

        assert!(nested.exists());
        assert_eq!(log.list(10).unwrap().len(), 1);
    }
}

#[cfg(test)]
mod growth_tests {
    use super::*;
    use chrono::Utc;

    fn record(id: usize) -> OperationRecord {
        OperationRecord {
            id: format!("op{id}"),
            at: Utc::now(),
            // Padded so the log reaches the compaction threshold without writing a million
            // entries in a test.
            title: format!("Clean {}", "x".repeat(400)),
            mode: crate::models::DeleteMode::Trash,
            status: crate::models::CleanupStatus::Success,
            removed_targets: 1,
            removed_files: 1,
            removed_bytes: id as u64,
            failed_count: 0,
            providers: vec![],
        }
    }

    #[test]
    fn the_log_stops_growing_and_keeps_the_newest_entries() {
        let dir = tempfile::tempdir().unwrap();
        let log = OperationLog::open(dir.path());

        // Enough to cross the size threshold several times over.
        let total = KEEP_RECORDS + 400;
        for i in 0..total {
            log.append(&record(i)).unwrap();
        }

        let size = std::fs::metadata(dir.path().join("operations.jsonl"))
            .unwrap()
            .len();
        assert!(size <= MAX_BYTES * 2, "the log kept growing: {size} bytes");

        // The most recent operation is still the first thing the user sees...
        let listed = log.list(5).unwrap();
        assert_eq!(listed[0].id, format!("op{}", total - 1));
        // ...and the oldest have been let go rather than kept forever.
        let all = log.list(usize::MAX).unwrap();
        assert!(all.len() <= KEEP_RECORDS + 400);
        assert!(!all.iter().any(|r| r.id == "op0"));
    }

    #[test]
    fn a_short_log_is_left_exactly_as_it_is() {
        let dir = tempfile::tempdir().unwrap();
        let log = OperationLog::open(dir.path());
        for i in 0..5 {
            log.append(&record(i)).unwrap();
        }

        let listed = log.list(10).unwrap();
        assert_eq!(listed.len(), 5);
        assert_eq!(listed[0].id, "op4");
        assert_eq!(listed[4].id, "op0");
        // No temporary file left behind.
        assert!(!dir.path().join("operations.jsonl.writing").exists());
    }
}
