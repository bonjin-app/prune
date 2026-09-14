//! IPC surface. Naming: `<domain>_<verb>_<object>` in `snake_case`.
//!
//! Domains: `app`, `system`, `cleaner`, `disk`, `apps`, `startup`, `ops`, `fs`.

mod app;
mod apps;
mod cleaner;
mod disk;
mod fs;
mod ops;
mod startup;
mod system;

pub use app::*;
pub use apps::*;
pub use cleaner::*;
pub use disk::*;
pub use fs::*;
pub use ops::*;
pub use startup::*;
pub use system::*;
