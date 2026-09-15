//! Reading Docker's output.
//!
//! `docker system df` reports sizes as human strings (`12.4GB`, `947.5kB`), not bytes, so they
//! have to be parsed back. Docker uses SI units, which is what Prune displays too, so the
//! numbers stay consistent with the rest of the app.
//!
//! Everything here is forgiving: an unrecognised row is skipped rather than failing the whole
//! reading, because a Docker version that adds a row must not break the view.

use serde::Deserialize;

use super::{DockerEntry, DockerKind, DockerUsage};

/// One line of `docker system df --format "{{json .}}"`. Every field arrives as a string.
#[derive(Debug, Deserialize)]
struct Row {
    #[serde(rename = "Type")]
    kind: String,
    #[serde(rename = "TotalCount", default)]
    total_count: String,
    #[serde(rename = "Active", default)]
    active: String,
    #[serde(rename = "Size", default)]
    size: String,
    #[serde(rename = "Reclaimable", default)]
    reclaimable: String,
}

fn kind_of(name: &str) -> Option<DockerKind> {
    match name.trim().to_ascii_lowercase().as_str() {
        "images" => Some(DockerKind::Images),
        "containers" => Some(DockerKind::Containers),
        "local volumes" | "volumes" => Some(DockerKind::Volumes),
        "build cache" => Some(DockerKind::BuildCache),
        _ => None,
    }
}

/// Parses a size as Docker writes it: `0B`, `947.5kB`, `12.4GB`, `1.5 TB`.
///
/// Returns 0 for anything unrecognised, which is the safe direction: a size Prune cannot read
/// is reported as nothing to reclaim rather than as a number it invented.
pub fn parse_size(text: &str) -> u64 {
    let text = text.trim();
    let digits_end = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(digits_end);
    let Ok(value) = number.parse::<f64>() else {
        return 0;
    };
    // Docker writes `kB` for kilobytes; compare case-insensitively and accept both.
    let multiplier = match unit.trim().to_ascii_lowercase().as_str() {
        "" | "b" => 1.0,
        "kb" => 1e3,
        "mb" => 1e6,
        "gb" => 1e9,
        "tb" => 1e12,
        "pb" => 1e15,
        // Binary units, in case a future Docker prints them.
        "kib" => 1024.0,
        "mib" => 1024.0 * 1024.0,
        "gib" => 1024.0 * 1024.0 * 1024.0,
        "tib" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return 0,
    };
    (value * multiplier) as u64
}

/// `Reclaimable` arrives as `9.1GB (73%)`; only the size matters.
fn parse_reclaimable(text: &str) -> u64 {
    parse_size(text.split('(').next().unwrap_or(text))
}

/// Builds the usage summary from the lines `docker system df` printed.
pub fn parse_usage(stdout: &str) -> DockerUsage {
    let mut entries = Vec::new();
    for line in stdout.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let Ok(row) = serde_json::from_str::<Row>(line) else {
            continue;
        };
        let Some(kind) = kind_of(&row.kind) else {
            continue;
        };
        entries.push(DockerEntry {
            kind,
            label: kind.label().to_string(),
            note: kind.note().to_string(),
            total_count: row.total_count.trim().parse().unwrap_or(0),
            active_count: row.active.trim().parse().unwrap_or(0),
            size_bytes: parse_size(&row.size),
            reclaimable_bytes: parse_reclaimable(&row.reclaimable),
            reclaimable_by_prune: kind.reclaimable_by_prune(),
        });
    }
    let total_bytes = entries.iter().map(|e| e.size_bytes).sum();
    let reclaimable_bytes = entries
        .iter()
        .filter(|e| e.reclaimable_by_prune)
        .map(|e| e.reclaimable_bytes)
        .sum();
    DockerUsage {
        entries,
        total_bytes,
        reclaimable_bytes,
    }
}

/// Pulls the figure out of Docker's `Total reclaimed space: 9.1GB` footer.
pub fn reclaimed_total(stdout: &str) -> u64 {
    stdout
        .lines()
        .rev()
        .find_map(|line| {
            line.split_once("Total reclaimed space:")
                .map(|(_, size)| parse_size(size))
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_sizes_docker_prints() {
        assert_eq!(parse_size("0B"), 0);
        assert_eq!(parse_size("947.5kB"), 947_500);
        assert_eq!(parse_size("12.4GB"), 12_400_000_000);
        assert_eq!(parse_size("1.5 TB"), 1_500_000_000_000);
        assert_eq!(parse_size("512"), 512);
        assert_eq!(parse_size("  4.2MB  "), 4_200_000);
    }

    #[test]
    fn accepts_binary_units_in_case_docker_changes() {
        assert_eq!(parse_size("1KiB"), 1024);
        assert_eq!(parse_size("2GiB"), 2 * 1024 * 1024 * 1024);
    }

    #[test]
    fn an_unreadable_size_counts_as_nothing_rather_than_a_guess() {
        assert_eq!(parse_size(""), 0);
        assert_eq!(parse_size("unknown"), 0);
        assert_eq!(parse_size("12.4ZB"), 0);
        assert_eq!(parse_size("N/A"), 0);
    }

    #[test]
    fn takes_the_size_out_of_a_reclaimable_percentage() {
        assert_eq!(parse_reclaimable("9.1GB (73%)"), 9_100_000_000);
        assert_eq!(parse_reclaimable("6.8GB"), 6_800_000_000);
        assert_eq!(parse_reclaimable("0B (0%)"), 0);
    }

    #[test]
    fn skips_rows_it_does_not_understand() {
        let stdout = r#"{"Type":"Images","TotalCount":"2","Active":"1","Size":"1GB","Reclaimable":"500MB (50%)"}
not json at all
{"Type":"Something New","TotalCount":"9","Active":"0","Size":"5GB","Reclaimable":"5GB"}"#;
        let usage = parse_usage(stdout);
        assert_eq!(usage.entries.len(), 1);
        assert_eq!(usage.total_bytes, 1_000_000_000);
    }

    #[test]
    fn handles_empty_output() {
        let usage = parse_usage("");
        assert_eq!(usage, DockerUsage::default());
    }

    #[test]
    fn accepts_either_name_for_volumes() {
        for name in ["Local Volumes", "Volumes"] {
            let line = format!(
                r#"{{"Type":"{name}","TotalCount":"1","Active":"1","Size":"1GB","Reclaimable":"1GB"}}"#
            );
            let usage = parse_usage(&line);
            assert_eq!(usage.entries[0].kind, DockerKind::Volumes);
            // Counted, but not part of what Prune offers to reclaim.
            assert_eq!(usage.reclaimable_bytes, 0);
        }
    }

    #[test]
    fn reads_the_reclaimed_footer() {
        assert_eq!(
            reclaimed_total("deleted: sha256:abc\n\nTotal reclaimed space: 9.1GB"),
            9_100_000_000
        );
        assert_eq!(reclaimed_total("Total reclaimed space: 0B"), 0);
        // No footer at all is 0, not a panic.
        assert_eq!(reclaimed_total("nothing to do"), 0);
    }

    #[test]
    fn counts_are_read_but_never_required() {
        let line = r#"{"Type":"Containers","TotalCount":"","Active":"x","Size":"2GB","Reclaimable":"1GB"}"#;
        let usage = parse_usage(line);
        assert_eq!(usage.entries[0].total_count, 0);
        assert_eq!(usage.entries[0].active_count, 0);
        assert_eq!(usage.entries[0].size_bytes, 2_000_000_000);
    }
}
