//! Cleanup providers: one per data source. Adding support for a new tool means adding one
//! provider (often a single [`SimpleProvider`] declaration) and registering it in
//! [`registry::default_providers`].

mod context;
mod registry;
mod simple;

pub mod browser;
pub mod developer;
pub mod system;

pub use context::{ScanContext, ScanOutput};
pub use registry::{default_providers, ProviderRegistry};
pub use simple::{Root, RootMode, SimpleProvider};

use crate::models::{Category, RiskLevel};
use crate::platform::KnownPaths;

/// A source of removable data.
pub trait CleanupProvider: Send + Sync {
    /// Stable identifier, `snake_case`. Part of every target id.
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn category(&self) -> Category;
    fn description(&self) -> &'static str;
    /// Risk assigned to targets unless the provider decides otherwise per target.
    fn default_risk(&self) -> RiskLevel;
    /// When two providers report overlapping paths the higher priority wins.
    /// Generic providers (e.g. "everything under ~/Library/Caches") use a low priority so that
    /// specialised providers (e.g. "npm cache") take precedence.
    fn priority(&self) -> u8 {
        50
    }
    /// Whether there is anything to look at on this machine (cheap, no traversal).
    fn is_available(&self, known: &KnownPaths) -> bool;
    /// Discover targets. Must honour `ctx.cancel` and report progress through `ctx`.
    fn scan(&self, ctx: &ScanContext<'_>) -> ScanOutput;
}
