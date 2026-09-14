//! Read-only listing of programs that start by themselves. Prototype of `prune startup`.
//!
//! ```bash
//! cargo run -p prune-core --example startup --release
//! ```
//!
//! Listing only: this example never enables or disables anything.

use prune_core::PruneEngine;

fn main() {
    let engine = PruneEngine::new();
    let items = match engine.startup_items() {
        Ok(items) => items,
        Err(e) => {
            eprintln!("{e}");
            return;
        }
    };
    println!("{} startup items\n", items.len());
    for i in &items {
        println!(
            "  {:<9} {:<13} {:<7} {:<34} {}",
            if i.enabled { "enabled" } else { "disabled" },
            format!("{:?}", i.trigger),
            format!("{:?}", i.scope),
            i.name.chars().take(34).collect::<String>(),
            i.command
                .clone()
                .unwrap_or_else(|| i.path.clone())
                .chars()
                .take(70)
                .collect::<String>()
        );
    }
    let on = items.iter().filter(|i| i.enabled).count();
    println!("\n{on} enabled, {} disabled", items.len() - on);
}
