//! Local, append-only operation log (JSON lines). Never leaves the machine.

use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::models::OperationRecord;
use crate::{PruneError, Result};

pub struct OperationLog {
    path: PathBuf,
    lock: Mutex<()>,
}

impl OperationLog {
    /// `dir` is created if needed. The log lives at `<dir>/operations.jsonl`.
    pub fn open(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir).map_err(|e| PruneError::io(dir, e))?;
        Ok(Self {
            path: dir.join("operations.jsonl"),
            lock: Mutex::new(()),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&self, record: &OperationRecord) -> Result<()> {
        let _guard = self.lock.lock().unwrap();
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
        let file = match std::fs::File::open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(PruneError::io(&self.path, e)),
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
        let log = OperationLog::open(dir.path()).unwrap();
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
