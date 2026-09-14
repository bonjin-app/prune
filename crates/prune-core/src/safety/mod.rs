//! The safety layer. Every path that reaches the deletion code passes through
//! [`SafetyPolicy::validate`], regardless of where it came from.
//!
//! The policy is a *whitelist*: removal is only possible inside `allowed_roots`
//! (the user's home directory and the temp directory), and even there a set of exact paths and
//! whole trees are off limits.

mod policy;

pub use policy::{SafetyPolicy, ValidatedPath};
