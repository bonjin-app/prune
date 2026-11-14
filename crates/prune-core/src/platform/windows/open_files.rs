//! Which candidates a running program holds open, as far as Windows can say cheaply.
//!
//! Windows enforces this itself: a file another program has open without allowing deletion
//! cannot be moved to the Recycle Bin or deleted, and neither can a folder holding one, so the
//! removal fails and the result reports it as kept. What is checked here is only what is
//! cheap to check honestly, before anyone is offered the item: a candidate that is itself a
//! file is opened for reading without sharing, which Windows refuses while any other handle
//! to it exists. Folders are not walked; opening every file inside every folder would cost as
//! much as the scan itself.

use std::collections::HashMap;
use std::os::windows::fs::OpenOptionsExt;
use std::path::PathBuf;

use crate::platform::InUse;

const ERROR_SHARING_VIOLATION: i32 = 32;
const ERROR_LOCK_VIOLATION: i32 = 33;

pub fn in_use(candidates: &[PathBuf]) -> InUse {
    let mut found = HashMap::new();
    for candidate in candidates {
        let is_file = std::fs::symlink_metadata(candidate).is_ok_and(|m| m.is_file());
        if !is_file {
            continue;
        }
        let opened = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(candidate);
        if let Err(e) = opened {
            if matches!(
                e.raw_os_error(),
                Some(ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION)
            ) {
                found.insert(candidate.clone(), None);
            }
        }
    }
    InUse::Known(found)
}
