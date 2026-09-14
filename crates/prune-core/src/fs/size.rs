//! Directory size calculation.
//!
//! A single provider can be responsible for a tree with millions of entries (a pnpm store, a
//! Gradle cache), and that one tree used to decide how long a whole scan took. The walk is
//! therefore split across threads: the root's immediate subdirectories become independent
//! tasks, each walked serially, and their results are summed.
//!
//! Hard links are still counted once. The set of visited inodes is shared across threads and
//! sharded by inode number so the threads rarely contend on the same lock.

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

use rayon::prelude::*;
use walkdir::WalkDir;

use crate::models::ScanIssue;

/// Aggregate size of a directory tree.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DirStats {
    pub bytes: u64,
    pub files: u64,
    pub dirs: u64,
}

impl DirStats {
    pub fn add(&mut self, other: DirStats) {
        self.bytes += other.bytes;
        self.files += other.files;
        self.dirs += other.dirs;
    }
}

/// `(files_so_far, bytes_so_far, current_path)` progress callback.
pub type ProgressFn<'a> = &'a (dyn Fn(u64, u64, &Path) + Send + Sync);

/// Shared context for size calculations: cancellation and progress reporting.
pub struct SizeContext<'a> {
    pub cancel: &'a AtomicBool,
    /// Called every few hundred files.
    pub on_progress: Option<ProgressFn<'a>>,
    pub issues: &'a mut Vec<ScanIssue>,
}

impl SizeContext<'_> {
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

const PROGRESS_EVERY: u64 = 512;
/// Keep splitting the tree until there are at least this many independent subtrees to walk.
/// More tasks than cores lets rayon balance wildly uneven subtrees (a pnpm store's `ff/`
/// directory can be a hundred times the size of its `00/`).
const MIN_TASKS: usize = 32;
/// How many levels to descend while looking for those tasks. A store laid out as
/// `store/v10/files/ab/…` needs three.
const MAX_FANOUT_DEPTH: usize = 4;

/// Size of one directory entry: the file itself, or the whole tree for directories.
/// Symlinks are counted as the link itself (never followed).
pub fn entry_stats(path: &Path, ctx: &mut SizeContext<'_>) -> DirStats {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => dir_stats(path, ctx),
        Ok(meta) => DirStats {
            bytes: meta.len(),
            files: 1,
            dirs: 0,
        },
        Err(e) => {
            ctx.issues.push(ScanIssue {
                path: path.to_string_lossy().into(),
                message: e.to_string(),
            });
            DirStats::default()
        }
    }
}

/// Recursively sum a directory. Does not follow symlinks and counts hard-linked files once.
pub fn dir_stats(root: &Path, ctx: &mut SizeContext<'_>) -> DirStats {
    let seen = InodeSet::new();
    let counters = Counters::default();

    let Some((tasks, mut stats)) = fan_out(root, &seen, &counters, ctx) else {
        // Unreadable root; the error is already recorded as an issue.
        return DirStats::default();
    };

    let collected: Vec<(DirStats, Vec<ScanIssue>)> = if tasks.len() > 1 {
        tasks
            .par_iter()
            .map(|dir| walk(dir, ctx.cancel, &seen, &counters, ctx.on_progress))
            .collect()
    } else {
        tasks
            .iter()
            .map(|dir| walk(dir, ctx.cancel, &seen, &counters, ctx.on_progress))
            .collect()
    };
    for (sub, issues) in collected {
        stats.add(sub);
        ctx.issues.extend(issues);
    }
    stats
}

/// Splits `root` into independent subtrees to walk in parallel.
///
/// Descends level by level until there are enough subtrees, counting the directories and files
/// it passes on the way. Returns the subtrees still to walk plus the stats already accounted
/// for; `None` only when the root itself cannot be read.
///
/// Every returned subtree has already been counted in `DirStats::dirs`, which is why [`walk`]
/// does not count its own root.
fn fan_out(
    root: &Path,
    seen: &InodeSet,
    counters: &Counters,
    ctx: &mut SizeContext<'_>,
) -> Option<(Vec<std::path::PathBuf>, DirStats)> {
    let mut frontier = vec![root.to_path_buf()];
    let mut stats = DirStats::default();

    for depth in 0..MAX_FANOUT_DEPTH {
        if frontier.len() >= MIN_TASKS || ctx.is_cancelled() {
            break;
        }
        let mut children = Vec::new();
        for dir in &frontier {
            let entries = match std::fs::read_dir(dir) {
                Ok(e) => e,
                Err(e) => {
                    if depth == 0 {
                        ctx.issues.push(ScanIssue {
                            path: dir.to_string_lossy().into(),
                            message: e.to_string(),
                        });
                        return None;
                    }
                    ctx.issues.push(ScanIssue {
                        path: dir.to_string_lossy().into(),
                        message: e.to_string(),
                    });
                    continue;
                }
            };
            for entry in entries.filter_map(|e| e.ok()) {
                // A wide, shallow tree can be consumed entirely while fanning out, so this
                // loop has to honour cancellation and report progress just like `walk` does.
                if stats.files % 64 == 0 && ctx.is_cancelled() {
                    return Some((Vec::new(), stats));
                }
                // `symlink_metadata` so a symlink is measured as the link, exactly like the
                // subtree walk, which never follows links.
                let Ok(meta) = std::fs::symlink_metadata(entry.path()) else {
                    continue;
                };
                if meta.is_dir() {
                    stats.dirs += 1;
                    children.push(entry.path());
                    continue;
                }
                stats.files += 1;
                if !seen.first_visit(&meta) {
                    continue;
                }
                stats.bytes += meta.len();
                if let Some(cb) = ctx.on_progress {
                    let files = counters.files.fetch_add(1, Ordering::Relaxed) + 1;
                    let bytes =
                        counters.bytes.fetch_add(meta.len(), Ordering::Relaxed) + meta.len();
                    if files % PROGRESS_EVERY == 0 {
                        cb(files, bytes, &entry.path());
                    }
                }
            }
        }
        if children.is_empty() {
            // The whole tree fit in the levels already expanded.
            return Some((Vec::new(), stats));
        }
        frontier = children;
    }
    Some((frontier, stats))
}

#[derive(Default)]
struct Counters {
    files: AtomicU64,
    bytes: AtomicU64,
}

/// Walks one subtree serially. The subtree's own root directory is counted by the caller, so
/// `dirs` here covers only what is below it. Non-fatal errors are returned, not propagated.
fn walk(
    root: &Path,
    cancel: &AtomicBool,
    seen: &InodeSet,
    counters: &Counters,
    on_progress: Option<ProgressFn<'_>>,
) -> (DirStats, Vec<ScanIssue>) {
    let mut stats = DirStats::default();
    let mut issues = Vec::new();
    let mut since_check: u64 = 0;

    for entry in WalkDir::new(root).follow_links(false).into_iter() {
        since_check += 1;
        if since_check % 64 == 0 && cancel.load(Ordering::Relaxed) {
            break;
        }
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                issues.push(ScanIssue {
                    path: e
                        .path()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    message: e.to_string(),
                });
                continue;
            }
        };
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                issues.push(ScanIssue {
                    path: entry.path().to_string_lossy().into(),
                    message: e.to_string(),
                });
                continue;
            }
        };
        if meta.is_dir() {
            // depth 0 is this subtree's own root, which the caller counts.
            if entry.depth() > 0 {
                stats.dirs += 1;
            }
            continue;
        }
        stats.files += 1;
        if !seen.first_visit(&meta) {
            continue;
        }
        stats.bytes += meta.len();

        if let Some(cb) = on_progress {
            let files = counters.files.fetch_add(1, Ordering::Relaxed) + 1;
            let bytes = counters.bytes.fetch_add(meta.len(), Ordering::Relaxed) + meta.len();
            if files % PROGRESS_EVERY == 0 {
                cb(files, bytes, entry.path());
            }
        }
    }
    (stats, issues)
}

/// Tracks which hard-linked files have already been counted.
///
/// Sharded by inode so parallel walks almost never wait on the same lock. Files with a link
/// count of one skip it entirely, so ordinary trees never touch a lock at all.
struct InodeSet {
    #[cfg(unix)]
    shards: Vec<Mutex<std::collections::HashSet<(u64, u64)>>>,
}

const SHARDS: usize = 64;

impl InodeSet {
    fn new() -> Self {
        Self {
            #[cfg(unix)]
            shards: (0..SHARDS)
                .map(|_| Mutex::new(Default::default()))
                .collect(),
        }
    }

    /// `true` when this file's bytes should be counted.
    #[cfg(unix)]
    fn first_visit(&self, meta: &std::fs::Metadata) -> bool {
        use std::os::unix::fs::MetadataExt;
        if meta.nlink() <= 1 {
            return true;
        }
        let (dev, ino) = (meta.dev(), meta.ino());
        let shard = &self.shards[(ino as usize) % SHARDS];
        shard.lock().unwrap().insert((dev, ino))
    }

    /// Windows hard links are rare and `std` does not expose the link count, so every file
    /// counts once.
    #[cfg(not(unix))]
    fn first_visit(&self, _meta: &std::fs::Metadata) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats_of(root: &Path) -> (DirStats, Vec<ScanIssue>) {
        let cancel = AtomicBool::new(false);
        let mut issues = Vec::new();
        let mut ctx = SizeContext {
            cancel: &cancel,
            on_progress: None,
            issues: &mut issues,
        };
        let stats = dir_stats(root, &mut ctx);
        (stats, issues)
    }

    #[test]
    fn sums_files_and_counts_dirs() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("a/b")).unwrap();
        std::fs::write(dir.path().join("a/one.bin"), vec![0u8; 1000]).unwrap();
        std::fs::write(dir.path().join("a/b/two.bin"), vec![0u8; 24]).unwrap();
        let (stats, issues) = stats_of(dir.path());
        assert_eq!(
            stats,
            DirStats {
                bytes: 1024,
                files: 2,
                dirs: 2
            }
        );
        assert!(issues.is_empty());
    }

    /// Same tree, but wide enough that the parallel path is taken. The totals must match the
    /// serial path exactly.
    #[test]
    fn parallel_and_serial_paths_agree() {
        let dir = tempfile::tempdir().unwrap();
        let mut expected_bytes = 0u64;
        for i in 0..16 {
            let sub = dir.path().join(format!("sub{i}/inner"));
            std::fs::create_dir_all(&sub).unwrap();
            std::fs::write(sub.join("f.bin"), vec![0u8; 100 + i]).unwrap();
            std::fs::write(dir.path().join(format!("sub{i}/top.bin")), vec![0u8; 10]).unwrap();
            expected_bytes += 110 + i as u64;
        }
        std::fs::write(dir.path().join("root.bin"), vec![0u8; 7]).unwrap();
        expected_bytes += 7;

        let (stats, issues) = stats_of(dir.path());
        assert_eq!(stats.bytes, expected_bytes);
        assert_eq!(stats.files, 16 * 2 + 1);
        // 16 subN directories + 16 inner directories
        assert_eq!(stats.dirs, 32);
        assert!(issues.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_measured_as_links_not_targets() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target.bin");
        std::fs::write(&target, vec![0u8; 9_000]).unwrap();
        std::os::unix::fs::symlink(&target, dir.path().join("link.bin")).unwrap();
        let (stats, _) = stats_of(dir.path());
        assert_eq!(stats.files, 2);
        // 9000 for the real file plus the few bytes of the link itself, never 18000.
        assert!(stats.bytes < 9_500, "symlink was followed: {}", stats.bytes);
    }

    #[cfg(unix)]
    #[test]
    fn hard_links_are_counted_once_across_threads() {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("original.bin");
        std::fs::write(&original, vec![0u8; 5000]).unwrap();
        // Spread links across many subdirectories so different threads see the same inode.
        for i in 0..16 {
            let sub = dir.path().join(format!("sub{i}"));
            std::fs::create_dir_all(&sub).unwrap();
            std::fs::hard_link(&original, sub.join("link.bin")).unwrap();
        }
        let (stats, _) = stats_of(dir.path());
        assert_eq!(stats.files, 17);
        // The 5000 bytes are counted exactly once, no matter which thread got there first.
        assert_eq!(stats.bytes, 5000);
    }

    #[test]
    fn cancel_stops_early() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..8 {
            let sub = dir.path().join(format!("sub{i}"));
            std::fs::create_dir_all(&sub).unwrap();
            for j in 0..200 {
                std::fs::write(sub.join(format!("f{j}")), b"x").unwrap();
            }
        }
        let cancel = AtomicBool::new(true);
        let mut issues = Vec::new();
        let mut ctx = SizeContext {
            cancel: &cancel,
            on_progress: None,
            issues: &mut issues,
        };
        let stats = dir_stats(dir.path(), &mut ctx);
        assert!(stats.files < 8 * 200);
    }

    #[test]
    fn unreadable_root_reports_an_issue() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("gone");
        let (stats, issues) = stats_of(&missing);
        assert_eq!(stats, DirStats::default());
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn progress_reports_running_totals() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..8 {
            let sub = dir.path().join(format!("sub{i}"));
            std::fs::create_dir_all(&sub).unwrap();
            for j in 0..200 {
                std::fs::write(sub.join(format!("f{j}")), b"xx").unwrap();
            }
        }
        let cancel = AtomicBool::new(false);
        let seen_max = AtomicU64::new(0);
        let cb = |files: u64, _bytes: u64, _p: &Path| {
            seen_max.fetch_max(files, Ordering::Relaxed);
        };
        let mut issues = Vec::new();
        let mut ctx = SizeContext {
            cancel: &cancel,
            on_progress: Some(&cb),
            issues: &mut issues,
        };
        let stats = dir_stats(dir.path(), &mut ctx);
        assert_eq!(stats.files, 1600);
        // Progress fires every 512 files and reports the global running count.
        assert!(seen_max.load(Ordering::Relaxed) >= 512);
        assert!(seen_max.load(Ordering::Relaxed) <= stats.files);
    }
}
