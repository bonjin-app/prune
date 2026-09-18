//! Prune on the command line.
//!
//! Every subcommand runs against the same [`PruneEngine`] the desktop app uses, so the safety
//! rules are identical: removal only inside the home and temp directories, protected paths
//! refused, a plan built and shown before anything happens, and each operation appended to the
//! local log.
//!
//! Two habits make the command line safe to use in a hurry:
//!
//! - `clean` is a dry run unless `--yes` is given. Printing what would go is the default.
//! - `clean` only ever selects `Safe` targets, or `Low` as well with `--include-low`. Anything
//!   riskier is listed by `scan` but can only be chosen item by item in the desktop app, where
//!   the path and risk are in front of you.

use std::io::Write;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use prune_core::analyzer;
use prune_core::docker as docker_api;
use prune_core::models::{DeleteMode, RiskLevel, ScanSession, StopMode};
use prune_core::ops::OperationLog;
use prune_core::scan::ScanRequest;
use prune_core::system::SystemMonitor;
use prune_core::PruneEngine;

mod render;

pub use render::human_bytes;

#[derive(Debug, Parser)]
#[command(
    name = "prune",
    version,
    about = "Keep what matters. Remove what doesn't.",
    long_about = "Prune finds caches, build leftovers and other regenerable data, shows you \
                  exactly what it found, and removes only what you approve."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Print machine-readable JSON instead of a table.
    #[arg(long, global = true)]
    pub json: bool,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Show what Prune can look at on this machine.
    Status,
    /// List the cleanup providers and whether they apply here.
    Providers,
    /// Look for removable data. Read-only.
    Scan {
        /// Limit to these provider ids (see `prune providers`).
        #[arg(long = "only", value_name = "ID", num_args = 1..)]
        only: Vec<String>,
    },
    /// Remove regenerable data. A dry run unless `--yes` is given.
    Clean {
        /// Limit to these provider ids.
        #[arg(long = "only", value_name = "ID", num_args = 1..)]
        only: Vec<String>,
        /// Also take low-risk items such as node_modules and build directories.
        #[arg(long)]
        include_low: bool,
        /// Delete immediately instead of moving to the trash. Cannot be undone.
        #[arg(long)]
        permanent: bool,
        /// Actually remove what the plan lists.
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Show where the space goes in a directory. Read-only.
    Disk {
        /// Directory to analyze. Defaults to your home directory.
        path: Option<String>,
        /// Only list files at least this big, in megabytes.
        #[arg(long, default_value_t = 1000)]
        large: u64,
    },
    /// List installed applications, or one application's leftovers. Read-only.
    Apps {
        /// Show what this application leaves outside its own bundle.
        #[arg(long, value_name = "NAME")]
        detail: Option<String>,
    },
    /// Show what Docker is holding, or ask it to reclaim space.
    Docker {
        /// Remove build cache: `docker builder prune --force`.
        #[arg(long)]
        prune_cache: bool,
        /// Remove stopped containers, unused networks, dangling images and build cache:
        /// `docker system prune --force`. Volumes are never touched.
        #[arg(long)]
        prune_unused: bool,
        /// Actually run it. Without this the command is only printed.
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// List running processes, or stop one.
    Processes {
        /// How many to show, busiest first.
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Stop this process id. Prune refuses system processes and other users'.
        #[arg(long, value_name = "PID")]
        stop: Option<u32>,
        /// Stop it immediately instead of asking it to exit. Unsaved work is lost.
        #[arg(long)]
        force: bool,
    },
    /// List programs that start by themselves. Read-only.
    Startup,
    /// Show the local operation log.
    Log {
        /// How many entries to show.
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
}

/// Exit codes. `0` success, `1` a problem, `2` nothing matched.
pub const EXIT_OK: i32 = 0;
pub const EXIT_ERROR: i32 = 1;
pub const EXIT_NOTHING: i32 = 2;

/// Runs one command. `out` receives everything printed, which is what makes this testable.
pub fn run(
    engine: &PruneEngine,
    log: Option<&OperationLog>,
    cli: &Cli,
    out: &mut impl Write,
) -> std::io::Result<i32> {
    match &cli.command {
        Command::Status => status(engine, cli.json, out),
        Command::Providers => providers(engine, cli.json, out),
        Command::Scan { only } => {
            let session = scan(engine, only, out, !cli.json)?;
            if cli.json {
                write_json(out, &session)?;
            } else {
                render::scan_table(out, engine, &session)?;
            }
            Ok(if session.total_bytes == 0 {
                EXIT_NOTHING
            } else {
                EXIT_OK
            })
        }
        Command::Clean {
            only,
            include_low,
            permanent,
            yes,
        } => clean(
            engine,
            log,
            only,
            *include_low,
            *permanent,
            *yes,
            cli.json,
            out,
        ),
        Command::Disk { path, large } => disk(engine, path.as_deref(), *large, cli.json, out),
        Command::Apps { detail } => apps(engine, detail.as_deref(), cli.json, out),
        Command::Docker {
            prune_cache,
            prune_unused,
            yes,
        } => docker(*prune_cache, *prune_unused, *yes, cli.json, out),
        Command::Processes { limit, stop, force } => {
            processes(*limit, *stop, *force, cli.json, out)
        }
        Command::Startup => startup(engine, cli.json, out),
        Command::Log { limit } => operation_log(log, *limit, cli.json, out),
    }
}

fn write_json(out: &mut impl Write, value: &impl serde::Serialize) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    writeln!(out, "{text}")
}

fn status(engine: &PruneEngine, json: bool, out: &mut impl Write) -> std::io::Result<i32> {
    let known = engine.known_paths();
    let permissions = engine.permissions();
    if json {
        write_json(
            out,
            &serde_json::json!({
                "version": prune_core::VERSION,
                "platform": engine.platform().platform(),
                "home": known.home,
                "projectRoots": known.project_roots,
                "permissions": permissions,
            }),
        )?;
        return Ok(EXIT_OK);
    }
    writeln!(out, "Prune {}", prune_core::VERSION)?;
    writeln!(out, "  home           {}", known.home.display())?;
    writeln!(
        out,
        "  project roots  {}",
        render::join_paths(&known.project_roots)
    )?;
    if permissions.blocked.is_empty() {
        writeln!(out, "  permissions    everything Prune needs")?;
    } else {
        writeln!(
            out,
            "  permissions    cannot read {} ({})",
            permissions.blocked.join(", "),
            permissions.how_to_grant.unwrap_or_default()
        )?;
    }
    Ok(EXIT_OK)
}

fn providers(engine: &PruneEngine, json: bool, out: &mut impl Write) -> std::io::Result<i32> {
    let infos = engine.providers();
    if json {
        write_json(out, &infos)?;
        return Ok(EXIT_OK);
    }
    for info in &infos {
        writeln!(
            out,
            "{:<22} {:<10} {:<8} {}",
            info.id,
            render::risk(info.default_risk),
            if info.available { "present" } else { "-" },
            info.name
        )?;
    }
    Ok(EXIT_OK)
}

fn scan(
    engine: &PruneEngine,
    only: &[String],
    out: &mut impl Write,
    progress: bool,
) -> std::io::Result<ScanSession> {
    if progress {
        writeln!(out, "Scanning…")?;
        out.flush()?;
    }
    let request = ScanRequest {
        provider_ids: (!only.is_empty()).then(|| only.to_vec()),
    };
    Ok(engine.scan("cli", &request, Arc::new(AtomicBool::new(false)), &|_| {}))
}

/// Ids of the targets `clean` is willing to select without the user naming each one.
///
/// Risk decides most of it, but there is a second rule: some targets cannot be moved to the
/// trash — items already in it, for instance — and are always deleted outright. Selecting those
/// while promising "move to the trash" would delete a user's Trash on a command that reads as
/// reversible, so they are only taken when `--permanent` was asked for.
pub fn auto_selected(session: &ScanSession, include_low: bool, permanent: bool) -> Vec<String> {
    session
        .results
        .iter()
        .flat_map(|r| r.targets.iter())
        .filter(|t| t.risk == RiskLevel::Safe || (include_low && t.risk == RiskLevel::Low))
        .filter(|t| permanent || !t.permanent_only)
        .map(|t| t.id.clone())
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn clean(
    engine: &PruneEngine,
    log: Option<&OperationLog>,
    only: &[String],
    include_low: bool,
    permanent: bool,
    yes: bool,
    json: bool,
    out: &mut impl Write,
) -> std::io::Result<i32> {
    let session = scan(engine, only, out, !json)?;
    let ids = auto_selected(&session, include_low, permanent);
    let mode = if permanent {
        DeleteMode::Permanent
    } else {
        DeleteMode::Trash
    };
    let plan = engine.plan(&session, &ids, mode);

    // Targets that can only be deleted outright are left out of a trash-mode run. Saying so
    // matters most when they were the only thing found, or the answer looks like "nothing is
    // there" when something is.
    let held_back = if permanent {
        0
    } else {
        session
            .results
            .iter()
            .flat_map(|r| r.targets.iter())
            .filter(|t| t.permanent_only && (t.risk == RiskLevel::Safe || include_low))
            .count()
    };
    let held_back_note = |out: &mut dyn Write| -> std::io::Result<()> {
        if held_back == 0 {
            return Ok(());
        }
        writeln!(
            out,
            "\n{held_back} item(s) can only be deleted outright, such as anything already in \
             the trash. Add --permanent to include them."
        )
    };

    if plan.targets.is_empty() {
        if json {
            write_json(out, &plan)?;
        } else {
            writeln!(out, "Nothing to remove.")?;
            held_back_note(out)?;
        }
        return Ok(EXIT_NOTHING);
    }

    if !yes {
        if json {
            write_json(out, &plan)?;
        } else {
            render::plan_table(out, &plan)?;
            held_back_note(out)?;
            writeln!(
                out,
                "\nDry run. Nothing was removed. Add --yes to {}.",
                if permanent {
                    "delete these permanently"
                } else {
                    "move these to the trash"
                }
            )?;
        }
        return Ok(EXIT_OK);
    }

    let result = match engine.execute(&plan, log, &|_| {}) {
        Ok(result) => result,
        Err(e) => {
            writeln!(out, "error: {e}")?;
            return Ok(EXIT_ERROR);
        }
    };

    if json {
        write_json(out, &result)?;
    } else {
        writeln!(
            out,
            "Removed {} in {} files ({}).",
            human_bytes(result.removed_bytes),
            result.removed_files,
            if permanent {
                "deleted"
            } else {
                "moved to the trash"
            }
        )?;
        for failure in &result.failed {
            writeln!(out, "  kept {}: {}", failure.path, failure.error)?;
        }
    }
    Ok(if result.failed.is_empty() {
        EXIT_OK
    } else {
        EXIT_ERROR
    })
}

fn disk(
    engine: &PruneEngine,
    path: Option<&str>,
    large_mb: u64,
    json: bool,
    out: &mut impl Write,
) -> std::io::Result<i32> {
    let root = match path {
        Some(p) => std::path::PathBuf::from(p),
        None => engine.known_paths().home.clone(),
    };
    if !root.is_dir() {
        writeln!(out, "error: {} is not a directory", root.display())?;
        return Ok(EXIT_ERROR);
    }
    if !json {
        writeln!(out, "Analyzing {}…", root.display())?;
        out.flush()?;
    }
    let analysis = analyzer::analyze(
        "cli",
        &root,
        engine.policy(),
        &AtomicBool::new(false),
        &|_| {},
    );
    let summary = analysis.summary();
    let min_bytes = large_mb.saturating_mul(1_000_000);

    if json {
        write_json(
            out,
            &serde_json::json!({
                "summary": summary,
                "tree": analysis.node(None),
                "largeFiles": analysis.large_files(min_bytes, 100),
            }),
        )?;
        return Ok(EXIT_OK);
    }
    render::disk_report(out, &analysis, &summary, min_bytes)?;
    Ok(EXIT_OK)
}

fn apps(
    engine: &PruneEngine,
    detail: Option<&str>,
    json: bool,
    out: &mut impl Write,
) -> std::io::Result<i32> {
    let apps = match engine.applications() {
        Ok(apps) => apps,
        Err(e) => {
            writeln!(out, "error: {e}")?;
            return Ok(EXIT_ERROR);
        }
    };

    let Some(wanted) = detail else {
        if json {
            write_json(out, &apps)?;
        } else {
            for app in &apps {
                writeln!(
                    out,
                    "{:<38} {:<14} {}",
                    render::truncate(&app.name, 38),
                    app.version.clone().unwrap_or_default(),
                    if app.is_system { "system" } else { "" }
                )?;
            }
        }
        return Ok(EXIT_OK);
    };

    let needle = wanted.to_lowercase();
    let Some(app) = apps.iter().find(|a| a.name.to_lowercase() == needle) else {
        writeln!(out, "No application named {wanted}.")?;
        return Ok(EXIT_NOTHING);
    };
    let detail = engine.app_detail(app, &AtomicBool::new(false));
    if json {
        write_json(out, &detail)?;
    } else {
        render::app_detail(out, &detail)?;
    }
    Ok(EXIT_OK)
}

fn docker(
    prune_cache: bool,
    prune_unused: bool,
    yes: bool,
    json: bool,
    out: &mut impl Write,
) -> std::io::Result<i32> {
    let runner = docker_api::SystemRunner;

    let action = match (prune_unused, prune_cache) {
        (true, _) => Some(docker_api::DockerAction::SystemPrune),
        (_, true) => Some(docker_api::DockerAction::BuilderPrune),
        _ => None,
    };

    if let Some(action) = action {
        if !yes {
            writeln!(out, "{}", action.description())?;
            writeln!(out, "\nWould run: {}", action.display())?;
            writeln!(out, "Nothing was removed. Add --yes to run it.")?;
            return Ok(EXIT_OK);
        }
        return match docker_api::prune(&runner, action) {
            Ok(result) => {
                if json {
                    write_json(out, &result)?;
                } else {
                    writeln!(
                        out,
                        "{} reclaimed {}.",
                        result.command,
                        human_bytes(result.reclaimed_bytes)
                    )?;
                }
                Ok(EXIT_OK)
            }
            Err(e) => {
                writeln!(out, "error: {e}")?;
                Ok(EXIT_ERROR)
            }
        };
    }

    let state = docker_api::status(&runner);
    if json {
        write_json(out, &state)?;
        return Ok(match state {
            docker_api::DockerState::Ready { .. } => EXIT_OK,
            _ => EXIT_NOTHING,
        });
    }
    match state {
        docker_api::DockerState::NotInstalled => {
            writeln!(out, "Docker is not installed.")?;
            Ok(EXIT_NOTHING)
        }
        docker_api::DockerState::NotRunning { message } => {
            writeln!(out, "Docker is not answering: {message}")?;
            Ok(EXIT_NOTHING)
        }
        docker_api::DockerState::Ready { usage } => {
            for entry in &usage.entries {
                writeln!(
                    out,
                    "{:<14} {:>10}  {:>10} reclaimable  {:>3} of {:>3} in use{}",
                    entry.label,
                    human_bytes(entry.size_bytes),
                    human_bytes(entry.reclaimable_bytes),
                    entry.active_count,
                    entry.total_count,
                    if entry.reclaimable_by_prune {
                        ""
                    } else {
                        "  (kept: Prune never removes volumes)"
                    }
                )?;
            }
            writeln!(
                out,
                "\n{} in use, {} reclaimable with `prune docker --prune-unused`.",
                human_bytes(usage.total_bytes),
                human_bytes(usage.reclaimable_bytes)
            )?;
            Ok(EXIT_OK)
        }
    }
}

fn processes(
    limit: usize,
    stop: Option<u32>,
    force: bool,
    json: bool,
    out: &mut impl Write,
) -> std::io::Result<i32> {
    let monitor = SystemMonitor::new(&std::env::temp_dir());

    if let Some(pid) = stop {
        let mode = if force {
            StopMode::Force
        } else {
            StopMode::Ask
        };
        return match monitor.stop_process(pid, mode) {
            Ok(()) => {
                writeln!(
                    out,
                    "{} process {pid}.",
                    if force { "Stopped" } else { "Asked to quit" }
                )?;
                Ok(EXIT_OK)
            }
            Err(e) => {
                writeln!(out, "error: {e}")?;
                Ok(EXIT_ERROR)
            }
        };
    }

    // A process's CPU share is the work it did between two readings, so a single-shot command
    // has to take both. Without this every process reports 0.0% and the "busiest first" order
    // silently becomes "largest first", which is a different question than the one asked.
    let _ = monitor.processes(1);
    std::thread::sleep(prune_core::system::CPU_SAMPLE_INTERVAL);

    let list = monitor.processes(limit.min(2000));
    if json {
        write_json(out, &list)?;
        return Ok(EXIT_OK);
    }
    for p in &list {
        writeln!(
            out,
            "{:>8}  {:>6}  {:>10}  {:<30} {}",
            p.pid,
            format!("{:.1}%", p.cpu_percent),
            human_bytes(p.memory_bytes),
            render::truncate(&p.name, 30),
            if p.can_terminate {
                String::new()
            } else {
                format!(
                    "protected: {}",
                    p.protected_reason.clone().unwrap_or_default()
                )
            }
        )?;
    }
    Ok(EXIT_OK)
}

fn startup(engine: &PruneEngine, json: bool, out: &mut impl Write) -> std::io::Result<i32> {
    let items = match engine.startup_items() {
        Ok(items) => items,
        Err(e) => {
            writeln!(out, "error: {e}")?;
            return Ok(EXIT_ERROR);
        }
    };
    if json {
        write_json(out, &items)?;
        return Ok(EXIT_OK);
    }
    for item in &items {
        writeln!(
            out,
            "{:<9} {:<30} {}",
            if item.enabled { "enabled" } else { "disabled" },
            render::truncate(&item.name, 30),
            item.command.clone().unwrap_or_else(|| item.path.clone())
        )?;
    }
    Ok(EXIT_OK)
}

fn operation_log(
    log: Option<&OperationLog>,
    limit: usize,
    json: bool,
    out: &mut impl Write,
) -> std::io::Result<i32> {
    let Some(log) = log else {
        writeln!(out, "No operation log on this machine yet.")?;
        return Ok(EXIT_NOTHING);
    };
    let records = log.list(limit).unwrap_or_default();
    if json {
        write_json(out, &records)?;
        return Ok(EXIT_OK);
    }
    if records.is_empty() {
        writeln!(out, "No cleanups recorded yet.")?;
        return Ok(EXIT_NOTHING);
    }
    for record in &records {
        writeln!(
            out,
            "{}  {:>10}  {:<28} {}",
            record.at.format("%Y-%m-%d %H:%M"),
            human_bytes(record.removed_bytes),
            render::truncate(&record.title, 28),
            match record.mode {
                DeleteMode::Trash => "trash",
                DeleteMode::Permanent => "permanent",
            }
        )?;
    }
    Ok(EXIT_OK)
}
