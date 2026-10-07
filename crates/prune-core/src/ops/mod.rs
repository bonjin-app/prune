//! Local, append-only operation log (JSON lines). Never leaves the machine.

use std::fs::OpenOptions;
use std::io::Write;
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
        let mut line = serde_json::to_string(record)?;
        line.push('\n');
        // A write cut short by a crash leaves a line with no newline. Appending straight onto it
        // would make one corrupt line out of that and this record, and this record — the one
        // that says what was just removed — would be the one lost.
        if self.ends_mid_line() {
            line.insert(0, '\n');
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| PruneError::io(&self.path, e))?;
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

    /// Whether the file is non-empty and its last byte is not a newline.
    fn ends_mid_line(&self) -> bool {
        use std::io::{Read, Seek, SeekFrom};
        let Ok(mut f) = std::fs::File::open(&self.path) else {
            return false;
        };
        let Ok(len) = f.metadata().map(|m| m.len()) else {
            return false;
        };
        if len == 0 || f.seek(SeekFrom::Start(len - 1)).is_err() {
            return false;
        }
        let mut last = [0u8; 1];
        f.read_exact(&mut last).is_ok() && last[0] != b'\n'
    }

    /// Every non-blank line of the file, as bytes.
    ///
    /// Bytes rather than text, because a line need not be text: a write interrupted inside a
    /// multi-byte character leaves one that is not valid UTF-8. Reading as text stopped at the
    /// first such line, which in a list that runs newest first discards exactly the entries
    /// someone opened it to see — and made compaction fail on every append thereafter.
    fn read_lines(&self) -> std::io::Result<Vec<Vec<u8>>> {
        let bytes = std::fs::read(&self.path)?;
        Ok(bytes
            .split(|b| *b == b'\n')
            .filter(|l| !l.iter().all(u8::is_ascii_whitespace))
            .map(<[u8]>::to_vec)
            .collect())
    }

    /// Rewrites the log with only the most recent entries.
    ///
    /// Written to a temporary file and renamed, so an interrupted compaction leaves the
    /// previous log intact rather than a truncated one.
    fn compact(&self) -> Result<()> {
        let lines = self
            .read_lines()
            .map_err(|e| PruneError::io(&self.path, e))?;
        if lines.len() <= KEEP_RECORDS {
            return Ok(());
        }
        let mut kept = lines[lines.len() - KEEP_RECORDS..].join(&b'\n');
        kept.push(b'\n');
        let temp = self.path.with_extension("jsonl.writing");
        std::fs::write(&temp, kept).map_err(|e| PruneError::io(&temp, e))?;
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
        let lines = match self.read_lines() {
            Ok(l) => l,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => {
                tracing::warn!(path = %self.path.display(), error = %e, "cannot read the operation log");
                return Ok(vec![]);
            }
        };
        let mut records: Vec<OperationRecord> = lines
            .iter()
            .filter_map(|l| serde_json::from_slice(l).ok())
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

#[cfg(test)]
mod damage_tests {
    use super::*;
    use chrono::Utc;

    fn record(id: &str) -> OperationRecord {
        OperationRecord {
            id: id.into(),
            at: Utc::now(),
            title: "Clean".into(),
            mode: crate::models::DeleteMode::Trash,
            status: crate::models::CleanupStatus::Success,
            removed_targets: 1,
            removed_files: 1,
            removed_bytes: 1,
            failed_count: 0,
            providers: vec![],
        }
    }

    fn ids(log: &OperationLog) -> Vec<String> {
        log.list(usize::MAX)
            .unwrap()
            .into_iter()
            .map(|r| r.id)
            .collect()
    }

    /// A line that is not text: what a write interrupted inside a multi-byte character leaves.
    const NOT_UTF8: &[u8] = &[b'{', b'"', b'i', b'd', b'"', b':', b'"', 0xE2, 0x82];

    #[test]
    fn a_line_that_is_not_text_does_not_hide_what_came_after_it() {
        // `list` is newest first, so what a stop at the first bad line throws away is exactly
        // what the person came to see.
        let dir = tempfile::tempdir().unwrap();
        let log = OperationLog::open(dir.path());
        log.append(&record("old")).unwrap();
        let mut f = OpenOptions::new().append(true).open(log.path()).unwrap();
        f.write_all(NOT_UTF8).unwrap();
        f.write_all(b"\n").unwrap();
        drop(f);
        log.append(&record("new")).unwrap();

        assert_eq!(ids(&log), ["new", "old"]);
    }

    #[test]
    fn an_unfinished_last_line_does_not_swallow_the_next_record() {
        // A crash can leave half a line with no newline. The next record was appended straight
        // onto it, making one corrupt line out of a good record and a bad one.
        let dir = tempfile::tempdir().unwrap();
        let log = OperationLog::open(dir.path());
        log.append(&record("before")).unwrap();
        let mut f = OpenOptions::new().append(true).open(log.path()).unwrap();
        f.write_all(br#"{"id":"half"#).unwrap();
        drop(f);
        log.append(&record("after")).unwrap();

        assert_eq!(ids(&log), ["after", "before"]);
    }

    #[test]
    fn a_log_with_damage_in_it_is_still_shortened() {
        // Compaction read the whole file as text, so one bad byte made it fail on every append
        // from then on, and the log grew for good.
        let dir = tempfile::tempdir().unwrap();
        let log = OperationLog::open(dir.path());
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log.path())
            .unwrap();
        f.write_all(NOT_UTF8).unwrap();
        f.write_all(b"\n").unwrap();
        drop(f);

        for i in 0..(KEEP_RECORDS + 1_500) {
            let mut r = record(&format!("op{i}"));
            r.title = format!("Clean {}", "x".repeat(400));
            log.append(&r).unwrap();
        }

        let size = std::fs::metadata(log.path()).unwrap().len();
        assert!(size <= MAX_BYTES * 2, "the log kept growing: {size} bytes");
        assert_eq!(
            log.list(1).unwrap()[0].id,
            format!("op{}", KEEP_RECORDS + 1_499)
        );
    }

    #[test]
    fn empty_and_unparseable_lines_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let log = OperationLog::open(dir.path());
        log.append(&record("a")).unwrap();
        let mut f = OpenOptions::new().append(true).open(log.path()).unwrap();
        f.write_all(b"\n\nnot json at all\n{\"id\": 3}\n").unwrap();
        drop(f);
        log.append(&record("b")).unwrap();

        assert_eq!(ids(&log), ["b", "a"]);
    }
}
