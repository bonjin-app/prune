//! Graphics processors, from their I/O registry entries (`ioreg -r -d 1 -c IOAccelerator`).
//!
//! The same read-only registry walk as the battery: no rights needed, nothing changed, about
//! 20 ms. Each accelerator publishes `PerformanceStatistics`, which is where Activity Monitor's
//! GPU history comes from too.

use std::process::Command;

use plist::{Dictionary, Value};

use crate::models::GpuStatus;

const IOREG: &str = "/usr/sbin/ioreg";

pub fn read() -> Vec<GpuStatus> {
    let Ok(output) = Command::new(IOREG)
        .args(["-r", "-d", "1", "-c", "IOAccelerator", "-a"])
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    parse(&output.stdout)
}

/// Reads `ioreg -a` output: an array with one entry per accelerator.
pub(crate) fn parse(bytes: &[u8]) -> Vec<GpuStatus> {
    let Ok(value) = Value::from_reader(std::io::Cursor::new(bytes)) else {
        return Vec::new();
    };
    let Some(entries) = value.as_array() else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(Value::as_dictionary)
        .map(gpu)
        .collect()
}

fn gpu(entry: &Dictionary) -> GpuStatus {
    let stats = entry
        .get("PerformanceStatistics")
        .and_then(Value::as_dictionary);
    let stat = |key: &str| stats.and_then(|s| number(s, key));
    // Apple silicon and Intel graphics say "Device Utilization %"; AMD cards "GPU Activity(%)".
    let usage_percent = stat("Device Utilization %")
        .or_else(|| stat("GPU Activity(%)"))
        .map(|p| p.min(100) as f32);
    // A discrete card reports its own memory, used and free; some AMD drivers name the used
    // figure `inUseVidMemoryBytes`. Apple silicon has none of its own and reports what it is
    // using of the shared memory.
    let vram_used = stat("vramUsedBytes").or_else(|| stat("inUseVidMemoryBytes"));
    let vram_free = stat("vramFreeBytes");
    let memory_used_bytes = vram_used.or_else(|| stat("In use system memory"));
    let memory_total_bytes = vram_used.zip(vram_free).map(|(u, f)| u + f);
    GpuStatus {
        name: entry
            .get("model")
            .and_then(|v| match v {
                Value::String(s) => Some(s.clone()),
                // Some drivers publish the model as raw bytes, NUL-terminated.
                Value::Data(d) => Some(String::from_utf8_lossy(d).trim_end_matches('\0').into()),
                _ => None,
            })
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        usage_percent,
        memory_used_bytes,
        memory_total_bytes,
        core_count: number(entry, "gpu-core-count").and_then(|n| u32::try_from(n).ok()),
    }
}

fn number(dict: &Dictionary, key: &str) -> Option<u64> {
    match dict.get(key)? {
        Value::Integer(i) => i.as_unsigned(),
        Value::Real(r) if *r >= 0.0 => Some(*r as u64),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plist(body: &str) -> Vec<u8> {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><array>{body}</array></plist>"#
        )
        .into_bytes()
    }

    #[test]
    fn reads_an_apple_silicon_gpu() {
        // Trimmed from an M-series Mac.
        let gpus = parse(&plist(
            r#"<dict>
              <key>model</key><string>Apple M5 Max</string>
              <key>gpu-core-count</key><integer>40</integer>
              <key>PerformanceStatistics</key><dict>
                <key>Device Utilization %</key><integer>17</integer>
                <key>In use system memory</key><integer>1302134784</integer>
              </dict>
            </dict>"#,
        ));
        assert_eq!(
            gpus,
            [GpuStatus {
                name: Some("Apple M5 Max".into()),
                usage_percent: Some(17.0),
                memory_used_bytes: Some(1_302_134_784),
                memory_total_bytes: None,
                core_count: Some(40),
            }]
        );
    }

    #[test]
    fn reads_a_discrete_card_and_leaves_what_it_omits_unknown() {
        let gpus = parse(&plist(
            r#"<dict>
              <key>PerformanceStatistics</key><dict>
                <key>GPU Activity(%)</key><integer>140</integer>
                <key>vramUsedBytes</key><integer>1000</integer>
                <key>vramFreeBytes</key><integer>3000</integer>
              </dict>
            </dict>
            <dict><key>IOClass</key><string>Something</string></dict>"#,
        ));
        assert_eq!(gpus.len(), 2);
        assert_eq!(gpus[0].usage_percent, Some(100.0), "capped at 100");
        assert_eq!(gpus[0].memory_used_bytes, Some(1000));
        assert_eq!(gpus[0].memory_total_bytes, Some(4000));
        assert_eq!(gpus[0].name, None);
        assert_eq!(gpus[1], GpuStatus::default());
    }

    #[test]
    fn reads_the_amd_name_for_memory_in_use() {
        let gpus = parse(&plist(
            r#"<dict><key>PerformanceStatistics</key><dict>
                <key>inUseVidMemoryBytes</key><integer>5000</integer>
            </dict></dict>"#,
        ));
        assert_eq!(gpus[0].memory_used_bytes, Some(5000));
    }

    #[test]
    fn unreadable_output_is_no_gpus() {
        assert!(parse(b"not a plist").is_empty());
        assert!(parse(&plist("")).is_empty());
    }
}
