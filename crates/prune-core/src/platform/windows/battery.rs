//! The battery, from `GetSystemPowerStatus`. One call into kernel32, no rights needed.
//!
//! Windows reports charge, mains power and time left here. Cycle count and wear need WMI and,
//! on many machines, a vendor driver, so they are left unknown rather than estimated.

use crate::models::BatteryStatus;

/// `SYSTEM_POWER_STATUS`, as declared in `winbase.h`.
#[repr(C)]
#[derive(Default)]
struct SystemPowerStatus {
    ac_line_status: u8,
    battery_flag: u8,
    battery_life_percent: u8,
    system_status_flag: u8,
    battery_life_time: u32,
    battery_full_life_time: u32,
}

#[link(name = "kernel32")]
extern "system" {
    fn GetSystemPowerStatus(status: *mut SystemPowerStatus) -> i32;
}

pub fn read() -> Option<BatteryStatus> {
    let mut status = SystemPowerStatus::default();
    // SAFETY: `status` is a valid, writable SYSTEM_POWER_STATUS for the duration of the call.
    let ok = unsafe { GetSystemPowerStatus(&mut status) };
    if ok == 0 {
        return None;
    }
    interpret(
        status.ac_line_status,
        status.battery_flag,
        status.battery_life_percent,
        status.battery_life_time,
    )
}

const NO_BATTERY: u8 = 128;
const UNKNOWN_STATUS: u8 = 255;
const CHARGING: u8 = 8;
const UNKNOWN_PERCENT: u8 = 255;
const UNKNOWN_SECONDS: u32 = u32::MAX;

/// The documented meaning of each field, kept apart from the call so it can be tested anywhere.
fn interpret(ac_line: u8, flag: u8, percent: u8, seconds_left: u32) -> Option<BatteryStatus> {
    if flag == UNKNOWN_STATUS || flag & NO_BATTERY != 0 || percent == UNKNOWN_PERCENT {
        return None;
    }
    let plugged_in = ac_line == 1;
    let charging = flag & CHARGING != 0;
    // Windows only estimates time left while discharging.
    let minutes_remaining = (!plugged_in && seconds_left != UNKNOWN_SECONDS && seconds_left > 0)
        .then_some(seconds_left / 60);
    Some(BatteryStatus {
        percent: f32::from(percent.min(100)),
        plugged_in,
        charging,
        minutes_remaining,
        cycle_count: None,
        design_cycle_count: None,
        capacity_percent: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_discharging_laptop() {
        let b = interpret(0, 1, 64, 5_400).unwrap();
        assert_eq!(b.percent, 64.0);
        assert!(!b.plugged_in && !b.charging);
        assert_eq!(b.minutes_remaining, Some(90));
    }

    #[test]
    fn reads_a_charging_laptop_without_inventing_a_time() {
        let b = interpret(1, CHARGING, 40, UNKNOWN_SECONDS).unwrap();
        assert!(b.plugged_in && b.charging);
        assert_eq!(b.minutes_remaining, None);
    }

    #[test]
    fn a_desktop_has_no_battery() {
        assert_eq!(
            interpret(1, NO_BATTERY, UNKNOWN_PERCENT, UNKNOWN_SECONDS),
            None
        );
        assert_eq!(interpret(255, UNKNOWN_STATUS, 50, 0), None);
    }
}
