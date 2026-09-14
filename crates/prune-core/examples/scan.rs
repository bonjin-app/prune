//! Read-only scan of the current machine. Prototype of the future `prune scan` CLI.
//!
//! ```bash
//! cargo run -p prune-core --example scan --release
//! ```
//!
//! Nothing is removed; this only prints what Prune *would* offer for review.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use prune_core::scan::ScanRequest;
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
    let known = engine.known_paths();
    println!("Prune {} — read-only scan", prune_core::VERSION);
    let permissions = engine.permissions();
    if !permissions.blocked.is_empty() {
        println!(
            "note: cannot read {} — {}",
            permissions.blocked.join(", "),
            permissions.how_to_grant.clone().unwrap_or_default()
        );
    }
    println!("home: {}", known.home.display());
    println!("project roots: {:?}\n", known.project_roots);

    // Optional provider filter, e.g. `--example scan -- pnpm_store npm_cache`.
    let only: Vec<String> = std::env::args().skip(1).collect();
    let request = ScanRequest {
        provider_ids: (!only.is_empty()).then_some(only),
    };

    let start = std::time::Instant::now();
    let session = engine.scan("cli", &request, Arc::new(AtomicBool::new(false)), &|_| {});

    for r in &session.results {
        if r.targets.is_empty() && r.issues.is_empty() {
            continue;
        }
        println!(
            "{:<28} {:>10}  {:>5} items  {:>6} ms  {}",
            r.provider_name,
            human(r.total_bytes),
            r.targets.len(),
            r.duration_ms,
            if r.issues.is_empty() {
                String::new()
            } else {
                format!("({} skipped)", r.issues.len())
            }
        );
        let mut top: Vec<_> = r.targets.iter().collect();
        top.sort_by_key(|t| std::cmp::Reverse(t.size_bytes));
        for t in top.iter().take(3) {
            println!(
                "    {:>10}  {:<10?} {}",
                human(t.size_bytes),
                t.risk,
                t.path
            );
        }
    }
    println!(
        "\nTotal reclaimable: {} in {} files ({} providers, {:.1}s)",
        human(session.total_bytes),
        session.total_files,
        session.results.len(),
        start.elapsed().as_secs_f32()
    );
}
