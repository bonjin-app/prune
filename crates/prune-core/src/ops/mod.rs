//! Local, append-only operation log (JSON lines). Never leaves the machine.

use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::models::OperationRecord;
use crate::{PruneError, Result};

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
        let _guard = self.lock.lock().unwrap();
        std::fs::create_dir_all(&self.dir).map_err(|e| PruneError::io(&self.dir, e))?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| PruneError::io(&self.path, e))?;
        let mut line = serde_json::to_string(record)?;
        line.push('\n');
        file.write_all(line.as_bytes())
            .map_err(|e| PruneError::io(&self.path, e))
    }

    /// Most recent first. Corrupt lines are skipped.
    pub fn list(&self, limit: usize) -> Result<Vec<OperationRecord>> {
        let _guard = self.lock.lock().unwrap();
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
