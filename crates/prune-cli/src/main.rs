use std::io::Write;
use std::process::ExitCode;

use clap::Parser;
use prune_cli::{Cli, Command, EXIT_ERROR};
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

    // Editing the protected list needs the settings file, not an engine. It also reports an
    // unreadable file itself, as a refusal rather than a warning.
    if let Command::Protect { action } = &cli.command {
        let cwd = std::env::current_dir().unwrap_or_default();
        let home = dirs::home_dir();
        let stdout = std::io::stdout();
        let mut out = std::io::BufWriter::new(stdout.lock());
        let code = prune_cli::protect::run(
            dir.as_deref(),
            home.as_deref(),
            &cwd,
            action.as_ref(),
            cli.json,
            &mut out,
        )
        .unwrap_or(EXIT_ERROR);
        let _ = out.flush();
        return ExitCode::from(code as u8);
    }
    let loaded = dir
        .as_deref()
        .map(Settings::load_checked)
        .unwrap_or_default();
    let mut engine = match dir.as_deref() {
        Some(dir) => PruneEngine::configured(&loaded.settings, dir),
        None => PruneEngine::with_settings(&loaded.settings),
    };
    // The settings hold what the user protected. Unreadable, that is unknown, and the command
    // line removes nothing until the app has saved them again.
    if let Some(problem) = &loaded.problem {
        eprintln!("warning: {problem}");
        engine.refuse_all_removal(problem.clone());
    }
    for issue in engine.custom_provider_issues() {
        eprintln!(
            "warning: providers.json: {}{}",
            issue
                .id
                .as_deref()
                .map(|id| format!("{id}: "))
                .unwrap_or_default(),
            issue.message
        );
    }
    let log = dir.as_deref().map(OperationLog::open);

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
