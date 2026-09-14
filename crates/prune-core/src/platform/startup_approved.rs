//! The byte format Windows uses to remember which startup entries a user switched off.
//!
//! `StartupApproved\Run` and `StartupApproved\StartupFolder` store one binary value per entry:
//! twelve bytes whose first byte carries the state and whose tail is a timestamp. Task Manager
//! writes `0x02` when an entry is enabled and `0x03` when it is disabled; Windows has also been
//! seen writing `0x06` for enabled and `0x07`/`0x09` for disabled, so the state is read from the
//! low bit rather than from an exact value.
//!
//! This module is deliberately not gated behind `cfg(windows)`: it is pure byte math, so it can
//! be tested on any host. The Windows registry code is a thin wrapper around it.

/// Length Windows writes. Shorter values are padded, longer ones keep their tail.
pub const VALUE_LEN: usize = 12;

/// Reads the enabled state out of a `StartupApproved` value.
///
/// `None` when the value is empty, which callers treat as "enabled" because an entry with no
/// record has never been switched off.
pub fn is_enabled(bytes: &[u8]) -> Option<bool> {
    bytes.first().map(|b| b % 2 == 0)
}

/// Produces the value to write for the requested state, preserving anything Windows put in the
/// bytes beyond the state flag.
pub fn with_state(existing: Option<&[u8]>, enabled: bool) -> Vec<u8> {
    let mut bytes = existing.map(|b| b.to_vec()).unwrap_or_default();
    if bytes.len() < VALUE_LEN {
        bytes.resize(VALUE_LEN, 0);
    }
    bytes[0] = if enabled { 2 } else { 3 };
    // Bytes 1..4 are reserved and always zero; 4..12 hold the FILETIME of the change. Zeroing
    // the timestamp is what Windows itself writes for an entry that was never toggled, and it
    // accepts the value on the next login.
    for b in bytes.iter_mut().take(VALUE_LEN).skip(1) {
        *b = 0;
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_states_windows_writes() {
        // Task Manager's own values.
        assert_eq!(
            is_enabled(&[2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            Some(true)
        );
        assert_eq!(
            is_enabled(&[3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            Some(false)
        );
        // Values Windows has been observed writing after a toggle.
        assert_eq!(is_enabled(&[6]), Some(true));
        assert_eq!(is_enabled(&[7]), Some(false));
        assert_eq!(is_enabled(&[9]), Some(false));
        // No record at all: the caller decides, and treats it as enabled.
        assert_eq!(is_enabled(&[]), None);
    }

    #[test]
    fn writes_a_full_length_value() {
        let disabled = with_state(None, false);
        assert_eq!(disabled.len(), VALUE_LEN);
        assert_eq!(is_enabled(&disabled), Some(false));

        let enabled = with_state(None, true);
        assert_eq!(enabled.len(), VALUE_LEN);
        assert_eq!(is_enabled(&enabled), Some(true));
    }

    #[test]
    fn round_trips_through_both_states() {
        let mut value = with_state(None, true);
        for expected in [false, true, false] {
            value = with_state(Some(&value), expected);
            assert_eq!(is_enabled(&value), Some(expected));
        }
    }

    #[test]
    fn pads_short_values_and_keeps_longer_tails() {
        let short = with_state(Some(&[3, 9]), true);
        assert_eq!(short.len(), VALUE_LEN);
        assert_eq!(is_enabled(&short), Some(true));

        let long = with_state(Some(&[3; 20]), false);
        assert_eq!(long.len(), 20, "a longer value keeps its tail");
        assert_eq!(is_enabled(&long), Some(false));
        assert_eq!(&long[1..VALUE_LEN], &[0u8; VALUE_LEN - 1]);
    }

    #[test]
    fn clears_the_timestamp_so_windows_accepts_the_change() {
        let stale = [2u8, 0, 0, 0, 0xde, 0xad, 0xbe, 0xef, 1, 2, 3, 4];
        let written = with_state(Some(&stale), false);
        assert_eq!(&written[4..VALUE_LEN], &[0u8; 8]);
    }
}
