//! Read-only disk analysis of a directory. Prototype of the future `prune disk` CLI.
//!
//! ```bash
//! cargo run -p prune-core --example disk --release -- ~/Projects
//! ```

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use prune_core::analyzer;
use prune_core::PruneEngine;

fn human(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1000.0 && u < UNITS.len() - 1 {
        v /= 1000.0;
        u += 1;
    }
    if u == 0 {
        format!("{bytes} B")
    } else {
        format!("{v:.2} {}", UNITS[u])
    }
}

fn main() {
    let engine = PruneEngine::new();
    let root: PathBuf = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| engine.known_paths().home.clone());
    println!("Analyzing {} (read-only)\n", root.display());
    let analysis = analyzer::analyze(
        "cli",
        &root,
        engine.policy(),
        &AtomicBool::new(false),
        &|_| {},
    );
    let summary = analysis.summary();
    let view = analysis.node(None).expect("root node");

    println!("{}", view.node.name);
    for c in view.children.iter().take(15) {
        let pct = if view.node.size_bytes > 0 {
            c.size_bytes as f64 / view.node.size_bytes as f64 * 100.0
        } else {
            0.0
        };
        println!("  {:>10}  {:>5.1}%  {}", human(c.size_bytes), pct, c.name);
    }
    println!("\nLargest files");
    for f in analysis.large_files(0, 10) {
        println!("  {:>10}  {:<10?} {}", human(f.size_bytes), f.risk, f.path);
    }
    println!("\nFile types");
    for e in summary.top_extensions.iter().take(10) {
        println!(
            "  {:>10}  {:>8} files  .{}",
            human(e.bytes),
            e.count,
            e.extension
        );
    }
    println!(
        "\n{} in {} files, {} folders — {:.1}s, {} skipped",
        human(summary.total_bytes),
        summary.file_count,
        summary.dir_count,
        summary.duration_ms as f64 / 1000.0,
        summary.issue_count
    );
}
