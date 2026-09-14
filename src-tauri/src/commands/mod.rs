//! IPC surface. Naming: `<domain>_<verb>_<object>` in `snake_case`.
//!
//! Domains: `app`, `system`, `cleaner`, `disk`, `ops`, `fs`.

mod app;
mod cleaner;
mod disk;
mod fs;
mod ops;
mod system;

pub use app::*;
pub use cleaner::*;
pub use disk::*;
pub use fs::*;
pub use ops::*;
pub use system::*;
