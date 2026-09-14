//! # prune-core
//!
//! The engine behind Prune. Everything in this crate runs locally, never talks to the
//! network, and is UI-agnostic so it can back both the Tauri desktop app and a future CLI.
//!
//! ## Layers
//!
//! ```text
//! providers  → discover cleanup candidates (one provider per data source)
//! scan       → run providers in parallel with progress + cancellation
//! safety     → path validation, protected paths, risk classification
//! fs         → size calculation, trash / permanent deletion
//! ops        → local operation log (JSON lines)
//! platform   → OS specific knowledge (well-known paths, protected paths)
//! system     → CPU / memory / disk / process information
//! ```
//!
//! ## Safety pipeline
//!
//! ```text
//! Discovery → Path Validation → Risk Classification → User Confirmation
//!           → Dry Run (CleanupPlan) → Delete → Operation Log
//! ```
//!
//! Callers never hand raw paths to the deletion layer. They pass *target ids* produced by a
//! scan, which are resolved to paths inside the engine and validated again right before removal.

pub mod engine;
pub mod error;
pub mod fs;
pub mod models;
pub mod ops;
pub mod platform;
pub mod providers;
pub mod safety;
pub mod scan;
pub mod system;

pub use engine::PruneEngine;
pub use error::{PruneError, Result};

/// Semantic version of the core crate, embedded at compile time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
