//! Serializable data models shared between the core, the desktop app and (later) the CLI.
//!
//! All structs serialize with `camelCase` so the TypeScript side mirrors them 1:1
//! (see `src/types/` in the frontend).

mod cleanup;
mod risk;
mod scan;
mod system;

pub use cleanup::*;
pub use risk::*;
pub use scan::*;
pub use system::*;
