//! The battery, from the power manager's registry entry (`ioreg -rn AppleSmartBattery`).
//!
//! `ioreg` only reads the I/O registry: it needs no rights, changes nothing and takes about
//! 15 ms. It is run from its absolute path so nothing on `PATH` can stand in for it.

use std::process::Command;

use plist::{Dictionary, Value};

use crate::models::BatteryStatus;

const IOREG: &str = "/usr/sbin/ioreg";

/// What the OS reports for "still estimating" in its minute counters.
const UNKNOWN_MINUTES: u64 = 65_535;

pub fn read() -> Option<BatteryStatus> {
    let output = Command::new(IOREG)
        .args(["-rn", "AppleSmartBattery", "-a"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse(&output.stdout)
}

/// Reads `ioreg -a` output: a property list holding an array with the battery's entry, or an
/// empty one on a Mac without a battery.
pub(crate) fn parse(bytes: &[u8]) -> Option<BatteryStatus> {
    let value = Value::from_reader(std::io::Cursor::new(bytes)).ok()?;
    let entry = value.as_array()?.first()?.as_dictionary()?;
    if entry.get("BatteryInstalled").and_then(Value::as_boolean) == Some(false) {
        return None;
    }

    // On Apple silicon both are percentages (100/100); on Intel both are mAh. The ratio is the
    // charge either way.
    let current = number(entry, "CurrentCapacity")?;
    let max = number(entry, "MaxCapacity").filter(|m| *m > 0)?;
    let percent = ((current as f64 / max as f64) * 100.0).clamp(0.0, 100.0) as f32;

    let plugged_in = flag(entry, "ExternalConnected").unwrap_or(false);
    let charging = flag(entry, "IsCharging").unwrap_or(false);
    let minutes = |key: &str| number(entry, key).filter(|m| *m > 0 && *m < UNKNOWN_MINUTES);
    let minutes_remaining = if charging {
        minutes("AvgTimeToFull")
    } else if !plugged_in {
        minutes("AvgTimeToEmpty")
    } else {
        None
    };

    // Health: what a full charge holds now against what it held new. Newer macOS keeps the
    // figures in `BatteryData`; older and Intel Macs at the top level.
    let data = entry.get("BatteryData").and_then(Value::as_dictionary);
    let either = |key: &str| {
        data.and_then(|d| number(d, key))
            .or_else(|| number(entry, key))
    };
    let design = either("DesignCapacity").filter(|d| *d > 0);
    let holds = either("NominalChargeCapacity").or_else(|| number(entry, "AppleRawMaxCapacity"));
    let capacity_percent = match (holds, design) {
        (Some(holds), Some(design)) => {
            Some(((holds as f64 / design as f64) * 100.0).clamp(0.0, 100.0) as f32)
        }
        _ => None,
    };

    Some(BatteryStatus {
        percent,
        plugged_in,
        charging,
        minutes_remaining: minutes_remaining.map(|m| m as u32),
        cycle_count: number(entry, "CycleCount").map(|c| c as u32),
        design_cycle_count: number(entry, "DesignCycleCount9C").map(|c| c as u32),
        capacity_percent,
    })
}

fn number(dict: &Dictionary, key: &str) -> Option<u64> {
    let value = dict.get(key)?;
    value.as_unsigned_integer().or_else(|| {
        value
            .as_signed_integer()
            .and_then(|n| u64::try_from(n).ok())
    })
}

fn flag(dict: &Dictionary, key: &str) -> Option<bool> {
    dict.get(key).and_then(Value::as_boolean)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plist(entries: &str) -> Vec<u8> {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><array>{entries}</array></plist>"#
        )
        .into_bytes()
    }

    /// The shape an Apple silicon MacBook reports, trimmed to what is read.
    const APPLE_SILICON: &str = r#"<dict>
        <key>BatteryInstalled</key><true/>
        <key>CurrentCapacity</key><integer>80</integer>
        <key>MaxCapacity</key><integer>100</integer>
        <key>ExternalConnected</key><false/>
        <key>IsCharging</key><false/>
        <key>AvgTimeToEmpty</key><integer>312</integer>
        <key>AvgTimeToFull</key><integer>65535</integer>
        <key>CycleCount</key><integer>135</integer>
        <key>DesignCycleCount9C</key><integer>1000</integer>
        <key>BatteryData</key><dict>
          <key>DesignCapacity</key><integer>6249</integer>
          <key>NominalChargeCapacity</key><integer>5777</integer>
        </dict>
      </dict>"#;

    #[test]
    fn reads_an_apple_silicon_battery() {
        let b = parse(&plist(APPLE_SILICON)).unwrap();
        assert_eq!(b.percent, 80.0);
        assert!(!b.plugged_in && !b.charging);
        assert_eq!(b.minutes_remaining, Some(312));
        assert_eq!(b.cycle_count, Some(135));
        assert_eq!(b.design_cycle_count, Some(1000));
        let capacity = b.capacity_percent.unwrap();
        assert!((92.0..93.0).contains(&capacity), "{capacity}");
    }

    #[test]
    fn reads_time_to_full_while_charging_and_nothing_while_held_on_mains() {
        let charging = APPLE_SILICON
            .replace(
                "<key>ExternalConnected</key><false/>",
                "<key>ExternalConnected</key><true/>",
            )
            .replace(
                "<key>IsCharging</key><false/>",
                "<key>IsCharging</key><true/>",
            )
            .replace("<integer>65535</integer>", "<integer>18</integer>");
        let b = parse(&plist(&charging)).unwrap();
        assert!(b.plugged_in && b.charging);
        assert_eq!(b.minutes_remaining, Some(18));

        let held = APPLE_SILICON.replace(
            "<key>ExternalConnected</key><false/>",
            "<key>ExternalConnected</key><true/>",
        );
        let b = parse(&plist(&held)).unwrap();
        assert!(b.plugged_in && !b.charging);
        assert_eq!(b.minutes_remaining, None);
    }

    #[test]
    fn treats_the_estimating_marker_as_unknown() {
        let estimating = APPLE_SILICON.replace(
            "<key>AvgTimeToEmpty</key><integer>312</integer>",
            "<key>AvgTimeToEmpty</key><integer>65535</integer>",
        );
        assert_eq!(parse(&plist(&estimating)).unwrap().minutes_remaining, None);
    }

    #[test]
    fn reads_an_intel_battery_in_milliamp_hours() {
        let intel = r#"<dict>
            <key>CurrentCapacity</key><integer>3000</integer>
            <key>MaxCapacity</key><integer>6000</integer>
            <key>DesignCapacity</key><integer>7500</integer>
            <key>AppleRawMaxCapacity</key><integer>6000</integer>
            <key>ExternalConnected</key><true/>
            <key>IsCharging</key><true/>
            <key>CycleCount</key><integer>412</integer>
          </dict>"#;
        let b = parse(&plist(intel)).unwrap();
        assert_eq!(b.percent, 50.0);
        assert_eq!(b.capacity_percent, Some(80.0));
        assert_eq!(b.design_cycle_count, None);
    }

    #[test]
    fn a_mac_without_a_battery_has_none() {
        assert_eq!(parse(&plist("")), None);
        assert_eq!(
            parse(&plist("<dict><key>BatteryInstalled</key><false/></dict>")),
            None
        );
        assert_eq!(parse(b"not a plist"), None);
    }
}
