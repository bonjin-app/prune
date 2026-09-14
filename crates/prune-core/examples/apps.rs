//! Read-only listing of installed applications and the leftovers of one of them.
//!
//! ```bash
//! cargo run -p prune-core --example apps --release -- "Visual Studio Code"
//! ```

use std::sync::atomic::AtomicBool;

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
    let apps = engine.applications().expect("applications");
    println!("{} applications\n", apps.len());
    for a in apps.iter().take(60) {
        println!(
            "  {:<36} {:<12} {:<40} {}",
            a.name.chars().take(36).collect::<String>(),
            a.version.clone().unwrap_or_default(),
            a.bundle_id.clone().unwrap_or_default(),
            if a.is_system { "system" } else { "" }
        );
    }
    let wanted = std::env::args().nth(1);
    let Some(app) = apps
        .iter()
        .find(|a| wanted.as_deref().is_some_and(|w| a.name == w))
    else {
        return;
    };
    println!("\n{} — related data (read-only)", app.name);
    let detail = engine.app_detail(app, &AtomicBool::new(false));
    for item in &detail.items {
        println!(
            "  {:>10}  {:<20} {:<10?} {}",
            human(item.target.size_bytes),
            item.kind_label,
            item.target.risk,
            item.target.path
        );
    }
    println!(
        "\n  total {} · leftovers {}",
        human(detail.total_bytes),
        human(detail.leftover_bytes)
    );
}
