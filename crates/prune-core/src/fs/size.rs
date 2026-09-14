use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

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

impl<'a> SizeContext<'a> {
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

const PROGRESS_EVERY: u64 = 512;

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
    let mut stats = DirStats::default();
    #[cfg(unix)]
    let mut seen_inodes: HashSet<(u64, u64)> = HashSet::new();
    #[cfg(not(unix))]
    let _seen: HashSet<u64> = HashSet::new();

    let walker = WalkDir::new(root)
        .follow_links(false)
        .same_file_system(false)
        .into_iter();
    for entry in walker {
        if stats.files % 64 == 0 && ctx.is_cancelled() {
            break;
        }
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                let path = e
                    .path()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default();
                ctx.issues.push(ScanIssue {
                    path,
                    message: e.to_string(),
                });
                continue;
            }
        };
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                ctx.issues.push(ScanIssue {
                    path: entry.path().to_string_lossy().into(),
                    message: e.to_string(),
                });
                continue;
            }
        };
        if meta.is_dir() {
            if entry.depth() > 0 {
                stats.dirs += 1;
            }
            continue;
        }
        stats.files += 1;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if meta.nlink() > 1 && !seen_inodes.insert((meta.dev(), meta.ino())) {
                continue;
            }
        }
        stats.bytes += meta.len();
        if stats.files % PROGRESS_EVERY == 0 {
            if let Some(cb) = ctx.on_progress {
                cb(stats.files, stats.bytes, entry.path());
            }
        }
    }
    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_files_and_counts_dirs() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("a/b")).unwrap();
        std::fs::write(dir.path().join("a/one.bin"), vec![0u8; 1000]).unwrap();
        std::fs::write(dir.path().join("a/b/two.bin"), vec![0u8; 24]).unwrap();
        let cancel = AtomicBool::new(false);
        let mut issues = Vec::new();
        let mut ctx = SizeContext {
            cancel: &cancel,
            on_progress: None,
            issues: &mut issues,
        };
        let stats = dir_stats(dir.path(), &mut ctx);
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

    #[test]
    fn cancel_stops_early() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..200 {
            std::fs::write(dir.path().join(format!("f{i}")), b"x").unwrap();
        }
        let cancel = AtomicBool::new(true);
        let mut issues = Vec::new();
        let mut ctx = SizeContext {
            cancel: &cancel,
            on_progress: None,
            issues: &mut issues,
        };
        let stats = dir_stats(dir.path(), &mut ctx);
        assert!(stats.files < 200);
    }
}
