//! Batteries of connected wireless accessories, from two read-only sources:
//!
//! - the I/O registry (`ioreg -r -k BatteryPercent`), where Apple's own mice, keyboards and
//!   trackpads publish their charge; about 15 ms;
//! - `system_profiler SPBluetoothDataType`, which also covers headphones and the parts of
//!   earbuds (left, right, case); about 170 ms, and given at most [`PROFILER_LIMIT`].
//!
//! Neither needs rights or changes anything. Both are run from absolute paths.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use plist::Value;

use crate::models::AccessoryBattery;

const IOREG: &str = "/usr/sbin/ioreg";
const SYSTEM_PROFILER: &str = "/usr/sbin/system_profiler";

/// `system_profiler` can stall when Bluetooth is busy. A missing figure is better than a Monitor
/// that waits for it.
const PROFILER_LIMIT: Duration = Duration::from_secs(3);

pub fn read() -> Vec<AccessoryBattery> {
    let mut found = Command::new(IOREG)
        .args(["-r", "-k", "BatteryPercent", "-a"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| parse_registry(&o.stdout))
        .unwrap_or_default();
    if let Some(json) = run_with_limit(SYSTEM_PROFILER, &["SPBluetoothDataType", "-json"]) {
        merge(&mut found, parse_profiler(&json));
    }
    found
}

/// Accessories the registry did not already report. The registry wins for a device both know:
/// it is read more directly, and naming it twice would list it twice.
fn merge(found: &mut Vec<AccessoryBattery>, more: Vec<AccessoryBattery>) {
    // Checked against what the registry reported, not the growing list: an accessory the
    // registry did not know must keep every part (left, right, case), not only the first.
    let from_registry: Vec<String> = found.iter().map(|f| f.name.clone()).collect();
    found.extend(
        more.into_iter()
            .filter(|battery| !from_registry.contains(&battery.name)),
    );
}

fn run_with_limit(program: &str, args: &[&str]) -> Option<Vec<u8>> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    // Read on a thread so a large report cannot fill the pipe and stall the child.
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut stdout, &mut bytes).map(|_| bytes)
    });
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) | Err(_) => return None,
            Ok(None) if started.elapsed() > PROFILER_LIMIT => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
        }
    }
    reader.join().ok()?.ok()
}

/// Reads `ioreg -a` output: one entry per device that publishes `BatteryPercent`.
pub(crate) fn parse_registry(bytes: &[u8]) -> Vec<AccessoryBattery> {
    let Ok(value) = Value::from_reader(std::io::Cursor::new(bytes)) else {
        return Vec::new();
    };
    let mut found: Vec<AccessoryBattery> = Vec::new();
    for entry in value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_dictionary)
    {
        let Some(name) = entry
            .get("Product")
            .and_then(Value::as_string)
            .map(str::trim)
            .filter(|n| !n.is_empty())
        else {
            continue;
        };
        let Some(percent) = entry
            .get("BatteryPercent")
            .and_then(|v| v.as_unsigned_integer())
            .filter(|p| *p <= 100)
        else {
            continue;
        };
        // A device can publish the same figure on several of its interfaces.
        if found.iter().any(|f| f.name == name) {
            continue;
        }
        found.push(AccessoryBattery {
            name: name.to_string(),
            part: None,
            percent: percent as f32,
        });
    }
    found
}

/// Reads `system_profiler SPBluetoothDataType -json`: connected devices only, each with any of
/// `device_batteryLevelMain`, `…Left`, `…Right` and `…Case`, as text such as `"80%"`.
pub(crate) fn parse_profiler(bytes: &[u8]) -> Vec<AccessoryBattery> {
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    let controllers = json
        .get("SPBluetoothDataType")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten();
    for controller in controllers {
        let devices = controller
            .get("device_connected")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten();
        for device in devices.filter_map(|d| d.as_object()) {
            for (name, info) in device {
                for (key, part) in [
                    ("device_batteryLevelMain", None),
                    ("device_batteryLevel", None),
                    ("device_batteryLevelLeft", Some("left")),
                    ("device_batteryLevelRight", Some("right")),
                    ("device_batteryLevelCase", Some("case")),
                ] {
                    let Some(percent) = info.get(key).and_then(|v| v.as_str()).and_then(percent)
                    else {
                        continue;
                    };
                    if part.is_none() && found.iter().any(|f: &AccessoryBattery| &f.name == name) {
                        continue;
                    }
                    found.push(AccessoryBattery {
                        name: name.trim().to_string(),
                        part: part.map(str::to_string),
                        percent,
                    });
                }
            }
        }
    }
    found
}

fn percent(text: &str) -> Option<f32> {
    let value: f32 = text.trim().trim_end_matches('%').trim().parse().ok()?;
    (0.0..=100.0).contains(&value).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_apple_peripherals_from_the_registry_once_each() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><array>
  <dict><key>Product</key><string>Magic Mouse</string><key>BatteryPercent</key><integer>64</integer></dict>
  <dict><key>Product</key><string>Magic Mouse</string><key>BatteryPercent</key><integer>64</integer></dict>
  <dict><key>Product</key><string>Magic Keyboard</string><key>BatteryPercent</key><integer>100</integer></dict>
  <dict><key>Product</key><string>Odd</string><key>BatteryPercent</key><integer>255</integer></dict>
  <dict><key>BatteryPercent</key><integer>50</integer></dict>
</array></plist>"#;
        let found = parse_registry(xml.as_bytes());
        let names: Vec<(&str, f32)> = found.iter().map(|b| (b.name.as_str(), b.percent)).collect();
        assert_eq!(names, [("Magic Mouse", 64.0), ("Magic Keyboard", 100.0)]);
    }

    #[test]
    fn reads_earbuds_part_by_part_and_skips_what_is_not_connected() {
        let json = br#"{"SPBluetoothDataType":[{
          "device_connected":[
            {"AirPods Pro":{"device_batteryLevelLeft":"80%","device_batteryLevelRight":"79 %","device_batteryLevelCase":"50%"}},
            {"MX Master 3":{"device_batteryLevelMain":"35%"}},
            {"Speaker":{"device_minorType":"Speaker"}}
          ],
          "device_not_connected":[{"Old Mouse":{"device_batteryLevelMain":"10%"}}]
        }]}"#;
        let found = parse_profiler(json);
        let rows: Vec<(&str, Option<&str>, f32)> = found
            .iter()
            .map(|b| (b.name.as_str(), b.part.as_deref(), b.percent))
            .collect();
        assert_eq!(
            rows,
            [
                ("AirPods Pro", Some("left"), 80.0),
                ("AirPods Pro", Some("right"), 79.0),
                ("AirPods Pro", Some("case"), 50.0),
                ("MX Master 3", None, 35.0),
            ]
        );
    }

    #[test]
    fn the_registry_wins_for_a_device_both_report() {
        let mut found = vec![AccessoryBattery {
            name: "Magic Mouse".into(),
            part: None,
            percent: 64.0,
        }];
        merge(
            &mut found,
            vec![
                AccessoryBattery {
                    name: "Magic Mouse".into(),
                    part: None,
                    percent: 60.0,
                },
                AccessoryBattery {
                    name: "AirPods".into(),
                    part: Some("case".into()),
                    percent: 20.0,
                },
            ],
        );
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].percent, 64.0);
    }

    #[test]
    fn an_accessory_only_the_profiler_knows_keeps_every_part() {
        let part = |p: &str, percent: f32| AccessoryBattery {
            name: "AirPods Pro".into(),
            part: Some(p.into()),
            percent,
        };
        let mut found = Vec::new();
        merge(
            &mut found,
            vec![part("left", 80.0), part("right", 79.0), part("case", 50.0)],
        );
        let parts: Vec<_> = found.iter().map(|b| b.part.as_deref()).collect();
        assert_eq!(parts, [Some("left"), Some("right"), Some("case")]);
    }

    #[test]
    fn unreadable_output_is_nothing() {
        assert!(parse_registry(b"nope").is_empty());
        assert!(parse_profiler(b"nope").is_empty());
        assert_eq!(percent("101%"), None);
        assert_eq!(percent("n/a"), None);
    }

    #[test]
    fn reading_this_machine_never_fails() {
        for b in read() {
            assert!((0.0..=100.0).contains(&b.percent));
        }
    }
}
