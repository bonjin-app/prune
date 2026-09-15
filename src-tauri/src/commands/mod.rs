//! IPC surface. Naming: `<domain>_<verb>_<object>` in `snake_case`.
//!
//! Domains: `app`, `system`, `cleaner`, `disk`, `apps`, `docker`, `startup`, `settings`, `ops`,
//! `fs`.

mod app;
mod apps;
mod cleaner;
mod disk;
mod docker;
mod fs;
mod ops;
mod settings;
mod startup;
mod system;

pub use app::*;
pub use apps::*;
pub use cleaner::*;
pub use disk::*;
pub use docker::*;
pub use fs::*;
pub use ops::*;
pub use settings::*;
pub use startup::*;
pub use system::*;
