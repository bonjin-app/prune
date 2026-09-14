//! Filesystem primitives: size calculation and removal.
//!
//! Removal only accepts a [`ValidatedPath`](crate::safety::ValidatedPath), which can only be
//! produced by [`SafetyPolicy`](crate::safety::SafetyPolicy).

mod delete;
mod size;

pub use delete::{remove, RemoveOutcome};
pub use size::{dir_stats, entry_stats, DirStats, ProgressFn, SizeContext};
