//! Turning engine data into something readable in a terminal.

use std::io::Write;

use prune_core::analyzer::{DiskAnalysis, DiskSummary};
use prune_core::apps::AppDetail;
use prune_core::models::{CleanupPlan, RiskLevel, ScanResult, ScanSession};
use prune_core::PruneEngine;

/// Decimal units, matching Finder and Windows Settings.
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if bytes < 1000 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    let digits = if value >= 100.0 {
        0
    } else if value >= 10.0 {
        1
    } else {
        2
    };
    format!("{value:.digits$} {}", UNITS[unit], digits = digits)
}

pub fn risk(level: RiskLevel) -> &'static str {
    match level {
        RiskLevel::Safe => "safe",
        RiskLevel::Low => "low",
        RiskLevel::Medium => "medium",
        RiskLevel::High => "high",
        RiskLevel::Protected => "protected",
    }
}

/// Cuts a string to `width`, ending with `…` when something was dropped.
pub fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    let kept: String = text.chars().take(width.saturating_sub(1)).collect();
    format!("{kept}…")
}

pub fn join_paths(paths: &[std::path::PathBuf]) -> String {
    if paths.is_empty() {
        return "none found".to_string();
    }
    paths
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn scan_table(
    out: &mut impl Write,
    engine: &PruneEngine,
    session: &ScanSession,
) -> std::io::Result<()> {
    let mut any = false;
    for result in &session.results {
        if result.targets.is_empty() && result.issues.is_empty() {
            continue;
        }
        any = true;
        writeln!(
            out,
            "{:<26} {:>10}  {:>4} items{}",
            truncate(&result.provider_name, 26),
            human_bytes(result.total_bytes),
            result.targets.len(),
            if result.issues.is_empty() {
                String::new()
            } else {
                format!("  ({} skipped)", result.issues.len())
            }
        )?;
        if result.targets.iter().any(|t| t.group.is_some()) {
            grouped_rows(out, result)?;
        } else {
            let mut targets: Vec<_> = result.targets.iter().collect();
            targets.sort_by_key(|t| std::cmp::Reverse(t.size_bytes));
            for target in targets.iter().take(3) {
                writeln!(
                    out,
                    "    {:>10}  {:<10} {}",
                    human_bytes(target.size_bytes),
                    risk(target.risk),
                    target.path
                )?;
            }
            if targets.len() > 3 {
                writeln!(out, "    … and {} more", targets.len() - 3)?;
            }
        }
    }
    if !any {
        writeln!(out, "Nothing found.")?;
        return Ok(());
    }

    let permissions = engine.permissions();
    if !permissions.blocked.is_empty() {
        writeln!(
            out,
            "\nnote: {} are hidden from Prune. {}",
            permissions.blocked.join(", "),
            permissions.how_to_grant.unwrap_or_default()
        )?;
    }
    writeln!(
        out,
        "\nTotal {} in {} files.",
        human_bytes(session.total_bytes),
        session.total_files
    )
}

/// Rows for a provider whose targets know what project they belong to.
///
/// Hundreds of build directories in one list cannot be judged. One line per project, biggest
/// first, with how long since anyone worked on it, can be.
fn grouped_rows(out: &mut impl Write, result: &ScanResult) -> std::io::Result<()> {
    struct Project<'a> {
        label: &'a str,
        bytes: u64,
        items: usize,
        /// What the project is holding: node_modules, a Rust target, and so on.
        kinds: Vec<&'a str>,
        last_active: Option<chrono::DateTime<chrono::Utc>>,
    }

    let mut projects: Vec<Project<'_>> = Vec::new();
    let mut ungrouped = (0u64, 0usize);
    for target in &result.targets {
        let Some(group) = target.group.as_ref() else {
            ungrouped.0 += target.size_bytes;
            ungrouped.1 += 1;
            continue;
        };
        let kind = target.description.as_deref().unwrap_or("");
        match projects.iter_mut().find(|p| p.label == group.label) {
            Some(project) => {
                project.bytes += target.size_bytes;
                project.items += 1;
                if !kind.is_empty() && !project.kinds.contains(&kind) {
                    project.kinds.push(kind);
                }
            }
            None => projects.push(Project {
                label: &group.label,
                bytes: target.size_bytes,
                items: 1,
                kinds: if kind.is_empty() {
                    Vec::new()
                } else {
                    vec![kind]
                },
                last_active: group.last_active_at,
            }),
        }
    }
    projects.sort_by_key(|p| std::cmp::Reverse(p.bytes));

    let now = chrono::Utc::now();
    for project in projects.iter().take(8) {
        let age = match project.last_active {
            Some(at) => {
                let days = (now - at).num_days().max(0);
                format!("{days}d since work")
            }
            None => "no repository".to_string(),
        };
        let mut kinds = project.kinds.join(", ");
        if kinds.len() > 34 {
            kinds = truncate(&kinds, 34);
        }
        writeln!(
            out,
            "    {:>10}  {:>3} items  {:>16}  {:<24} {}",
            human_bytes(project.bytes),
            project.items,
            age,
            truncate(project.label, 24),
            kinds
        )?;
    }
    if projects.len() > 8 {
        writeln!(out, "    … and {} more projects", projects.len() - 8)?;
    }
    if ungrouped.1 > 0 {
        writeln!(
            out,
            "    {:>10}  {:>3} items  elsewhere",
            human_bytes(ungrouped.0),
            ungrouped.1
        )?;
    }
    Ok(())
}

pub fn plan_table(out: &mut impl Write, plan: &CleanupPlan) -> std::io::Result<()> {
    for target in &plan.targets {
        writeln!(
            out,
            "{:>10}  {:<10} {}",
            human_bytes(target.size_bytes),
            risk(target.risk),
            target.path
        )?;
    }
    for blocked in &plan.blocked {
        writeln!(out, "{:>10}  {:<10} {}", "-", "blocked", blocked.reason)?;
    }
    writeln!(
        out,
        "\nWould remove {} in {} files and {} directories.",
        human_bytes(plan.total_bytes),
        plan.file_count,
        plan.directory_count
    )
}

pub fn disk_report(
    out: &mut impl Write,
    analysis: &DiskAnalysis,
    summary: &DiskSummary,
    min_bytes: u64,
) -> std::io::Result<()> {
    if let Some(view) = analysis.node(None) {
        writeln!(out, "{}", view.node.path)?;
        for child in view.children.iter().take(15) {
            let share = if view.node.size_bytes > 0 {
                child.size_bytes as f64 / view.node.size_bytes as f64 * 100.0
            } else {
                0.0
            };
            writeln!(
                out,
                "  {:>10}  {:>5.1}%  {}",
                human_bytes(child.size_bytes),
                share,
                child.name
            )?;
        }
    }

    let large = analysis.large_files(min_bytes, 10);
    if !large.is_empty() {
        writeln!(out, "\nLargest files over {}", human_bytes(min_bytes))?;
        for file in &large {
            writeln!(
                out,
                "  {:>10}  {:<10} {}",
                human_bytes(file.size_bytes),
                risk(file.risk),
                file.path
            )?;
        }
    }

    if !summary.top_extensions.is_empty() {
        writeln!(out, "\nFile types")?;
        for ext in summary.top_extensions.iter().take(8) {
            writeln!(
                out,
                "  {:>10}  {:>8} files  .{}",
                human_bytes(ext.bytes),
                ext.count,
                ext.extension
            )?;
        }
    }

    writeln!(
        out,
        "\n{} in {} files and {} folders, {:.1}s{}.",
        human_bytes(summary.total_bytes),
        summary.file_count,
        summary.dir_count,
        summary.duration_ms as f64 / 1000.0,
        if summary.issue_count == 0 {
            String::new()
        } else {
            format!(", {} skipped", summary.issue_count)
        }
    )
}

pub fn app_detail(out: &mut impl Write, detail: &AppDetail) -> std::io::Result<()> {
    writeln!(
        out,
        "{} {}",
        detail.app.name,
        detail.app.version.clone().unwrap_or_default()
    )?;
    for item in &detail.items {
        writeln!(
            out,
            "  {:>10}  {:<20} {:<10} {}",
            human_bytes(item.target.size_bytes),
            truncate(&item.kind_label, 20),
            risk(item.target.risk),
            item.target.path
        )?;
    }
    writeln!(
        out,
        "\n  {} total, {} outside the application.",
        human_bytes(detail.total_bytes),
        human_bytes(detail.leftover_bytes)
    )?;
    if !detail.shared_with.is_empty() {
        writeln!(
            out,
            "\n  Another copy of this application is installed, and they share the same data:"
        )?;
        for path in &detail.shared_with {
            writeln!(out, "    {path}")?;
        }
        writeln!(
            out,
            "  The data above is left alone while that copy exists; removing it would take the\n  \
             other copy's settings with it. Remove the other copy first, then scan again."
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_sizes_in_decimal_units() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(999), "999 B");
        assert_eq!(human_bytes(1_000), "1.00 KB");
        assert_eq!(human_bytes(12_800_000_000), "12.8 GB");
        assert_eq!(human_bytes(382_000_000_000), "382 GB");
    }

    #[test]
    fn truncates_on_character_boundaries() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("exactly-ten", 11), "exactly-ten");
        assert_eq!(truncate("a-much-longer-name", 8), "a-much-…");
        // Multi-byte characters must not be cut in half.
        assert_eq!(truncate("한국어이름입니다", 4), "한국어…");
    }

    #[test]
    fn names_every_risk_level() {
        for level in [
            RiskLevel::Safe,
            RiskLevel::Low,
            RiskLevel::Medium,
            RiskLevel::High,
            RiskLevel::Protected,
        ] {
            assert!(!risk(level).is_empty());
        }
    }
}
