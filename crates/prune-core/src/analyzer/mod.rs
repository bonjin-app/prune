//! Disk analyzer: one walk over a directory tree producing
//!
//! * a directory tree with aggregated sizes (every directory, no files),
//! * the largest files,
//! * per-extension statistics.
//!
//! Read-only. Removal of large files goes through the normal cleanup pipeline: the analyzer
//! exposes them as [`CleanupTarget`]s so `PruneEngine::plan` / `execute` apply unchanged.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::models::{Category, CleanupTarget, RiskLevel, ScanIssue, ScanResult, TargetKind};
use crate::safety::SafetyPolicy;

/// Provider id used for large-file targets so they flow through the cleanup pipeline.
pub const LARGE_FILES_PROVIDER: &str = "large_files";

/// How many of the largest files to keep.
const LARGE_FILE_CAPACITY: usize = 2000;
const TOP_EXTENSIONS: usize = 40;
const PROGRESS_EVERY: u64 = 2048;

/// One directory in the analyzed tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskNode {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub file_count: u64,
    pub dir_count: u64,
    /// Number of direct child directories.
    pub child_dirs: usize,
    /// Bytes held by files directly inside this directory (not in subdirectories).
    pub own_file_bytes: u64,
}

/// A node plus its direct children, sorted by size descending.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskNodeView {
    pub node: DiskNode,
    pub children: Vec<DiskNode>,
    /// Ancestors from the scan root down to (excluding) this node.
    pub breadcrumbs: Vec<DiskNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LargeFile {
    /// Id usable with `cleaner_preview` (targets live in the synthetic large-files session).
    pub target_id: String,
    pub path: String,
    pub name: String,
    pub size_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_at: Option<DateTime<Utc>>,
    pub extension: String,
    pub risk: RiskLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionStat {
    pub extension: String,
    pub bytes: u64,
    pub count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiskScanStatus {
    Running,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskProgress {
    pub scan_id: String,
    pub files: u64,
    pub bytes: u64,
    pub dirs: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskSummary {
    pub scan_id: String,
    pub root: String,
    pub status: DiskScanStatus,
    pub total_bytes: u64,
    pub file_count: u64,
    pub dir_count: u64,
    pub duration_ms: u64,
    pub large_file_count: usize,
    pub issue_count: usize,
    pub top_extensions: Vec<ExtensionStat>,
    pub started_at: DateTime<Utc>,
}

struct NodeInner {
    name: String,
    path: PathBuf,
    parent: Option<usize>,
    size: u64,
    files: u64,
    dirs: u64,
    own_file_bytes: u64,
    children: Vec<usize>,
}

/// Result of [`analyze`]. Kept in memory by the app for drill-down queries.
pub struct DiskAnalysis {
    pub scan_id: String,
    pub root: PathBuf,
    pub status: DiskScanStatus,
    pub started_at: DateTime<Utc>,
    pub duration_ms: u64,
    nodes: Vec<NodeInner>,
    index: HashMap<PathBuf, usize>,
    large_files: Vec<LargeFile>,
    extensions: Vec<ExtensionStat>,
    pub issues: Vec<ScanIssue>,
}

impl DiskAnalysis {
    fn view_of(&self, idx: usize) -> DiskNode {
        let n = &self.nodes[idx];
        DiskNode {
            name: n.name.clone(),
            path: n.path.to_string_lossy().into_owned(),
            size_bytes: n.size,
            file_count: n.files,
            dir_count: n.dirs,
            child_dirs: n.children.len(),
            own_file_bytes: n.own_file_bytes,
        }
    }

    /// Node view for `path` (defaults to the root). `None` if the path was not scanned.
    ///
    /// The scan canonicalizes its root, so a caller-supplied path that goes through a symlink
    /// (`/var/...` vs `/private/var/...` on macOS) is canonicalized before the lookup.
    pub fn node(&self, path: Option<&Path>) -> Option<DiskNodeView> {
        let idx = match path {
            None => 0,
            Some(p) => match self.index.get(p) {
                Some(i) => *i,
                None => *self.index.get(&std::fs::canonicalize(p).ok()?)?,
            },
        };
        let mut children: Vec<DiskNode> = self.nodes[idx]
            .children
            .iter()
            .map(|&c| self.view_of(c))
            .collect();
        children.sort_by(|a, b| {
            b.size_bytes
                .cmp(&a.size_bytes)
                .then_with(|| a.name.cmp(&b.name))
        });
        let mut breadcrumbs = Vec::new();
        let mut cur = self.nodes[idx].parent;
        while let Some(p) = cur {
            breadcrumbs.push(self.view_of(p));
            cur = self.nodes[p].parent;
        }
        breadcrumbs.reverse();
        Some(DiskNodeView {
            node: self.view_of(idx),
            children,
            breadcrumbs,
        })
    }

    /// Largest files at least `min_bytes`, biggest first.
    pub fn large_files(&self, min_bytes: u64, limit: usize) -> Vec<LargeFile> {
        self.large_files
            .iter()
            .filter(|f| f.size_bytes >= min_bytes)
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn summary(&self) -> DiskSummary {
        let root = &self.nodes[0];
        DiskSummary {
            scan_id: self.scan_id.clone(),
            root: self.root.to_string_lossy().into_owned(),
            status: self.status,
            total_bytes: root.size,
            file_count: root.files,
            dir_count: root.dirs,
            duration_ms: self.duration_ms,
            large_file_count: self.large_files.len(),
            issue_count: self.issues.len(),
            top_extensions: self.extensions.clone(),
            started_at: self.started_at,
        }
    }

    /// The large files as a synthetic scan result so they can be planned and removed through
    /// the regular safety pipeline.
    pub fn large_files_result(&self) -> ScanResult {
        let targets: Vec<CleanupTarget> = self
            .large_files
            .iter()
            .map(|f| CleanupTarget {
                id: f.target_id.clone(),
                provider_id: LARGE_FILES_PROVIDER.into(),
                path: f.path.clone(),
                kind: TargetKind::File,
                size_bytes: f.size_bytes,
                file_count: 1,
                risk: f.risk,
                label: f.name.clone(),
                description: Some(f.extension.clone()),
                modified_at: f.modified_at,
                permanent_only: false,
            })
            .collect();
        let mut r = ScanResult {
            provider_id: LARGE_FILES_PROVIDER.into(),
            provider_name: "Large Files".into(),
            category: Category::LargeFiles,
            targets,
            total_bytes: 0,
            total_files: 0,
            duration_ms: self.duration_ms,
            issues: Vec::new(),
        };
        r.recompute_totals();
        r
    }

    /// Forget targets that were removed so later queries reflect reality (sizes of ancestor
    /// directories are adjusted too).
    pub fn forget_removed(&mut self, target_ids: &[String]) {
        let removed: Vec<LargeFile> = self
            .large_files
            .iter()
            .filter(|f| target_ids.contains(&f.target_id))
            .cloned()
            .collect();
        self.large_files
            .retain(|f| !target_ids.contains(&f.target_id));
        for f in removed {
            let path = PathBuf::from(&f.path);
            let mut dir = path.parent().and_then(|p| self.index.get(p).copied());
            let mut first = true;
            while let Some(idx) = dir {
                let n = &mut self.nodes[idx];
                n.size = n.size.saturating_sub(f.size_bytes);
                n.files = n.files.saturating_sub(1);
                if first {
                    n.own_file_bytes = n.own_file_bytes.saturating_sub(f.size_bytes);
                    first = false;
                }
                dir = n.parent;
            }
        }
    }
}

/// Directories skipped when scanning a filesystem root, to avoid other mounts and
/// APFS firmlink mirrors that would double count.
fn skip_when_root(root: &Path, path: &Path) -> bool {
    if root.parent().is_some() {
        return false;
    }
    let skip: &[&str] = if cfg!(target_os = "macos") {
        &[
            "/Volumes",
            "/System/Volumes",
            "/dev",
            "/private/var/vm",
            "/Network",
        ]
    } else if cfg!(target_os = "windows") {
        &[]
    } else {
        &["/proc", "/sys", "/dev", "/run", "/mnt", "/media"]
    };
    skip.iter().any(|s| path == Path::new(s))
}

/// Walk `root` and build a [`DiskAnalysis`]. Blocking; honours `cancel`.
pub fn analyze(
    scan_id: &str,
    root: &Path,
    policy: &SafetyPolicy,
    cancel: &AtomicBool,
    on_progress: &(dyn Fn(DiskProgress) + Send + Sync),
) -> DiskAnalysis {
    let started = Instant::now();
    let started_at = Utc::now();
    let root_name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.to_string_lossy().into_owned());

    let mut nodes: Vec<NodeInner> = vec![NodeInner {
        name: root_name,
        path: root.to_path_buf(),
        parent: None,
        size: 0,
        files: 0,
        dirs: 0,
        own_file_bytes: 0,
        children: Vec::new(),
    }];
    let mut index: HashMap<PathBuf, usize> = HashMap::new();
    index.insert(root.to_path_buf(), 0);
    // stack[d] = node index of the directory at depth d on the current DFS path
    let mut stack: Vec<usize> = vec![0];
    let mut heap: BinaryHeap<Reverse<(u64, usize)>> = BinaryHeap::new();
    let mut heap_entries: Vec<(PathBuf, u64, Option<DateTime<Utc>>)> = Vec::new();
    let mut extensions: HashMap<String, (u64, u64)> = HashMap::new();
    let mut issues: Vec<ScanIssue> = Vec::new();
    #[cfg(unix)]
    let mut seen_inodes: std::collections::HashSet<(u64, u64)> = std::collections::HashSet::new();
    let mut total_files: u64 = 0;
    let mut total_bytes: u64 = 0;
    let mut cancelled = false;

    let mut walker = WalkDir::new(root).follow_links(false).into_iter();
    while let Some(entry) = walker.next() {
        if total_files % 64 == 0 && cancel.load(Ordering::Relaxed) {
            cancelled = true;
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
                    message: e
                        .io_error()
                        .map(|io| io.to_string())
                        .unwrap_or_else(|| e.to_string()),
                });
                continue;
            }
        };
        let depth = entry.depth();
        if depth == 0 {
            continue;
        }
        let path = entry.path();
        if skip_when_root(root, path) {
            walker.skip_current_dir();
            continue;
        }
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                issues.push(ScanIssue {
                    path: path.to_string_lossy().into(),
                    message: e.to_string(),
                });
                continue;
            }
        };
        stack.truncate(depth);
        let parent = *stack.last().expect("stack never empty");

        if meta.is_dir() {
            let idx = nodes.len();
            nodes.push(NodeInner {
                name: entry.file_name().to_string_lossy().into_owned(),
                path: path.to_path_buf(),
                parent: Some(parent),
                size: 0,
                files: 0,
                dirs: 0,
                own_file_bytes: 0,
                children: Vec::new(),
            });
            nodes[parent].children.push(idx);
            index.insert(path.to_path_buf(), idx);
            stack.push(idx);
            // dir_count propagates to all ancestors
            let mut cur = Some(parent);
            while let Some(i) = cur {
                nodes[i].dirs += 1;
                cur = nodes[i].parent;
            }
            continue;
        }

        total_files += 1;
        #[cfg_attr(not(unix), allow(unused_mut))]
        let mut size = meta.len();
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if meta.nlink() > 1 && !seen_inodes.insert((meta.dev(), meta.ino())) {
                size = 0;
            }
        }
        total_bytes += size;
        nodes[parent].own_file_bytes += size;
        let mut cur = Some(parent);
        while let Some(i) = cur {
            nodes[i].size += size;
            nodes[i].files += 1;
            cur = nodes[i].parent;
        }

        {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase())
                .filter(|e| e.len() <= 12)
                .unwrap_or_else(|| "(none)".to_string());
            let e = extensions.entry(ext).or_insert((0, 0));
            e.0 += size;
            e.1 += 1;
        }

        if heap.len() < LARGE_FILE_CAPACITY || heap.peek().is_some_and(|Reverse((s, _))| size > *s)
        {
            let modified = meta.modified().ok().map(DateTime::<Utc>::from);
            heap_entries.push((path.to_path_buf(), size, modified));
            heap.push(Reverse((size, heap_entries.len() - 1)));
            if heap.len() > LARGE_FILE_CAPACITY {
                heap.pop();
            }
        }

        if total_files % PROGRESS_EVERY == 0 {
            on_progress(DiskProgress {
                scan_id: scan_id.to_string(),
                files: total_files,
                bytes: total_bytes,
                dirs: nodes.len() as u64 - 1,
                current_path: Some(path.to_string_lossy().into_owned()),
            });
        }
    }

    let mut large: Vec<LargeFile> = heap
        .into_iter()
        .map(|Reverse((_, i))| {
            let (path, size, modified) = &heap_entries[i];
            let risk = if policy.is_protected(path) {
                RiskLevel::Protected
            } else {
                RiskLevel::Medium
            };
            LargeFile {
                target_id: CleanupTarget::make_id(LARGE_FILES_PROVIDER, path),
                path: path.to_string_lossy().into_owned(),
                name: path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                size_bytes: *size,
                modified_at: *modified,
                extension: path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_ascii_lowercase())
                    .unwrap_or_default(),
                risk,
            }
        })
        .collect();
    large.sort_by(|a, b| {
        b.size_bytes
            .cmp(&a.size_bytes)
            .then_with(|| a.path.cmp(&b.path))
    });

    let mut ext_stats: Vec<ExtensionStat> = extensions
        .into_iter()
        .map(|(extension, (bytes, count))| ExtensionStat {
            extension,
            bytes,
            count,
        })
        .collect();
    ext_stats.sort_by_key(|e| std::cmp::Reverse(e.bytes));
    ext_stats.truncate(TOP_EXTENSIONS);

    let analysis = DiskAnalysis {
        scan_id: scan_id.to_string(),
        root: root.to_path_buf(),
        status: if cancelled {
            DiskScanStatus::Cancelled
        } else {
            DiskScanStatus::Completed
        },
        started_at,
        duration_ms: started.elapsed().as_millis() as u64,
        nodes,
        index,
        large_files: large,
        extensions: ext_stats,
        issues,
    };
    on_progress(DiskProgress {
        scan_id: scan_id.to_string(),
        files: total_files,
        bytes: total_bytes,
        dirs: analysis.nodes.len() as u64 - 1,
        current_path: None,
    });
    analysis
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::ProtectedPaths;

    fn write(path: &Path, size: usize) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, vec![0u8; size]).unwrap();
    }

    #[test]
    fn builds_tree_large_files_and_extensions() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("home");
        write(&root.join("a/big.iso"), 5000);
        write(&root.join("a/b/mid.zip"), 1500);
        write(&root.join("c/small.txt"), 10);
        write(&root.join("c/other.txt"), 20);
        write(&root.join("top.mov"), 700);
        let policy = SafetyPolicy::from_protected(ProtectedPaths {
            allowed_roots: vec![root.clone()],
            exact: vec![],
            trees: vec![root.join("c")],
            app_bundle_roots: vec![],
        });
        let cancel = AtomicBool::new(false);
        let analysis = analyze("t", &root, &policy, &cancel, &|_| {});

        let view = analysis.node(None).unwrap();
        assert_eq!(view.node.size_bytes, 5000 + 1500 + 10 + 20 + 700);
        assert_eq!(view.node.file_count, 5);
        assert_eq!(view.node.dir_count, 3);
        assert_eq!(view.node.own_file_bytes, 700);
        assert_eq!(view.children[0].name, "a");
        assert_eq!(view.children[0].size_bytes, 6500);
        assert_eq!(view.children[1].name, "c");

        // A path that reaches the same directory through a symlink resolves too: the scan
        // canonicalizes its root, so plain lookups would otherwise miss on macOS temp dirs.
        let b = analysis.node(Some(&root.join("a/b"))).unwrap();
        assert_eq!(b.node.size_bytes, 1500);
        assert_eq!(b.breadcrumbs.len(), 2);
        assert_eq!(b.breadcrumbs[1].name, "a");

        let large = analysis.large_files(1000, 10);
        assert_eq!(large.len(), 2);
        assert_eq!(large[0].name, "big.iso");
        assert_eq!(large[0].risk, RiskLevel::Medium);
        // files inside a protected tree are visible but Protected
        let all = analysis.large_files(0, 10);
        assert!(all
            .iter()
            .any(|f| f.name == "small.txt" && f.risk == RiskLevel::Protected));

        let summary = analysis.summary();
        assert_eq!(summary.top_extensions[0].extension, "iso");
        assert_eq!(
            summary
                .top_extensions
                .iter()
                .find(|e| e.extension == "txt")
                .unwrap()
                .count,
            2
        );
        assert_eq!(summary.status, DiskScanStatus::Completed);
    }

    #[test]
    fn forget_removed_adjusts_ancestors() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("home");
        write(&root.join("a/big.iso"), 5000);
        write(&root.join("a/small.bin"), 100);
        let policy = SafetyPolicy::from_protected(ProtectedPaths {
            allowed_roots: vec![root.clone()],
            ..Default::default()
        });
        let mut analysis = analyze("t", &root, &policy, &AtomicBool::new(false), &|_| {});
        let id = analysis.large_files(4000, 1)[0].target_id.clone();
        analysis.forget_removed(&[id]);
        assert_eq!(analysis.node(None).unwrap().node.size_bytes, 100);
        assert_eq!(
            analysis
                .node(Some(&root.join("a")))
                .unwrap()
                .node
                .file_count,
            1
        );
        assert!(analysis.large_files(4000, 1).is_empty());
    }

    #[test]
    fn cancel_marks_cancelled() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..100 {
            write(&dir.path().join(format!("f{i}")), 1);
        }
        let policy = SafetyPolicy::from_protected(ProtectedPaths::default());
        let analysis = analyze("t", dir.path(), &policy, &AtomicBool::new(true), &|_| {});
        assert_eq!(analysis.status, DiskScanStatus::Cancelled);
    }
}
