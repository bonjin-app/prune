use std::io::Write;
use std::process::ExitCode;

use clap::Parser;
use prune_cli::{Cli, EXIT_ERROR};
use prune_core::ops::OperationLog;
use prune_core::settings::Settings;
use prune_core::PruneEngine;

/// Where the desktop app keeps its settings and operation log, so both share one history.
fn data_dir() -> Option<std::path::PathBuf> {
    let base = if cfg!(target_os = "macos") {
        dirs::home_dir().map(|h| h.join("Library/Application Support"))
    } else {
        dirs::data_dir()
    };
    base.map(|d| d.join("app.bonjin.prune"))
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let dir = data_dir();
    let settings = dir.as_deref().map(Settings::load).unwrap_or_default();
    let engine = PruneEngine::with_settings(&settings);
    let log = dir.as_deref().and_then(|d| OperationLog::open(d).ok());

    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    let code = match prune_cli::run(&engine, log.as_ref(), &cli, &mut out) {
        Ok(code) => code,
        Err(e) => {
            let _ = writeln!(out, "error: {e}");
            EXIT_ERROR
        }
    };
    let _ = out.flush();
    ExitCode::from(code as u8)
}
