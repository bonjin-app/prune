//! Graphics processors, from the performance counters Task Manager's GPU graphs use:
//! `\GPU Engine(*)\Utilization Percentage` and `\GPU Adapter Memory(*)\Dedicated Usage`.
//!
//! Read through pdh.dll, which needs no rights and changes nothing. Utilization is a rate, so
//! the query is opened once and kept: every read measures the time since the one before.
//!
//! Windows counts each engine of each adapter per process. As Task Manager does, the processes
//! are added up per engine, and an adapter is as busy as its busiest engine. The counters do not
//! name the adapter, so its name is left unknown rather than guessed.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Duration;

use crate::models::GpuStatus;
use crate::sync::LockExt;

type PdhHandle = isize;

/// `PDH_FMT_COUNTERVALUE_ITEM_W` read as a double: the instance name, its status, the value.
#[repr(C)]
struct Item {
    name: *const u16,
    status: u32,
    value: f64,
}

#[link(name = "pdh")]
extern "system" {
    fn PdhOpenQueryW(source: *const u16, user: usize, query: *mut PdhHandle) -> u32;
    fn PdhAddEnglishCounterW(
        query: PdhHandle,
        path: *const u16,
        user: usize,
        counter: *mut PdhHandle,
    ) -> u32;
    fn PdhCollectQueryData(query: PdhHandle) -> u32;
    fn PdhGetFormattedCounterArrayW(
        counter: PdhHandle,
        format: u32,
        size: *mut u32,
        count: *mut u32,
        items: *mut Item,
    ) -> u32;
    fn PdhCloseQuery(query: PdhHandle) -> u32;
}

const OK: u32 = 0;
const PDH_MORE_DATA: u32 = 0x8000_07D2;
const PDH_FMT_DOUBLE: u32 = 0x0000_0200;
const PDH_FMT_NOCAP100: u32 = 0x0000_8000;
const PDH_CSTATUS_VALID_DATA: u32 = 0;
const PDH_CSTATUS_NEW_DATA: u32 = 1;

/// A rate needs two samples. The first read waits this long for its second one, once.
const FIRST_SAMPLE: Duration = Duration::from_millis(250);

struct Query {
    query: PdhHandle,
    engines: PdhHandle,
    /// 0 when the memory counter is missing; usage is still read.
    memory: PdhHandle,
}

// SAFETY: PDH handles are plain identifiers, usable from any thread; every use is under `STATE`.
unsafe impl Send for Query {}

impl Drop for Query {
    fn drop(&mut self) {
        // SAFETY: `query` came from PdhOpenQueryW and is closed exactly once.
        unsafe { PdhCloseQuery(self.query) };
    }
}

/// `None` until first used; `Some(None)` once it turned out there are no GPU counters.
static STATE: Mutex<Option<Option<Query>>> = Mutex::new(None);

pub fn read() -> Vec<GpuStatus> {
    let mut state = STATE.lock_recover();
    let query = state.get_or_insert_with(open);
    let Some(query) = query else {
        return Vec::new();
    };
    // SAFETY: `query.query` is an open query.
    if unsafe { PdhCollectQueryData(query.query) } != OK {
        return Vec::new();
    }
    let engines = values(query.engines, PDH_FMT_DOUBLE | PDH_FMT_NOCAP100);
    let memory = if query.memory == 0 {
        Vec::new()
    } else {
        values(query.memory, PDH_FMT_DOUBLE)
    };
    summarise(&engines, &memory)
}

fn open() -> Option<Query> {
    let mut query: PdhHandle = 0;
    // SAFETY: a null source means live data; `query` is written on success.
    if unsafe { PdhOpenQueryW(std::ptr::null(), 0, &mut query) } != OK {
        return None;
    }
    let add = |path: &str| -> PdhHandle {
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let mut counter: PdhHandle = 0;
        // SAFETY: `wide` is NUL-terminated and outlives the call; `counter` is written on success.
        let status = unsafe { PdhAddEnglishCounterW(query, wide.as_ptr(), 0, &mut counter) };
        if status == OK {
            counter
        } else {
            0
        }
    };
    let engines = add(r"\GPU Engine(*)\Utilization Percentage");
    let memory = add(r"\GPU Adapter Memory(*)\Dedicated Usage");
    let query = Query {
        query,
        engines,
        memory,
    };
    if engines == 0 {
        // Older Windows, or a machine without a WDDM 2 driver: there is nothing to read.
        return None;
    }
    // SAFETY: an open query.
    unsafe { PdhCollectQueryData(query.query) };
    std::thread::sleep(FIRST_SAMPLE);
    Some(query)
}

/// Every instance of a wildcard counter with a valid value, by instance name.
fn values(counter: PdhHandle, format: u32) -> Vec<(String, f64)> {
    let mut size = 0u32;
    let mut count = 0u32;
    // SAFETY: a null buffer with size 0 asks only for the size needed.
    let status = unsafe {
        PdhGetFormattedCounterArrayW(counter, format, &mut size, &mut count, std::ptr::null_mut())
    };
    if status != PDH_MORE_DATA || size == 0 {
        return Vec::new();
    }
    // u64 words so the items, which hold pointers and doubles, are aligned.
    let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
    // SAFETY: `buffer` holds at least `size` bytes; PDH writes the items and their names there.
    let status = unsafe {
        PdhGetFormattedCounterArrayW(
            counter,
            format,
            &mut size,
            &mut count,
            buffer.as_mut_ptr().cast::<Item>(),
        )
    };
    if status != OK {
        return Vec::new();
    }
    // SAFETY: PDH wrote `count` items at the start of `buffer`, whose names point into it.
    let items =
        unsafe { std::slice::from_raw_parts(buffer.as_ptr().cast::<Item>(), count as usize) };
    items
        .iter()
        .filter(|i| matches!(i.status, PDH_CSTATUS_VALID_DATA | PDH_CSTATUS_NEW_DATA))
        .filter(|i| !i.name.is_null())
        .map(|i| {
            // SAFETY: a NUL-terminated name inside `buffer`.
            let name = unsafe {
                let mut len = 0;
                while *i.name.add(len) != 0 {
                    len += 1;
                }
                String::from_utf16_lossy(std::slice::from_raw_parts(i.name, len))
            };
            (name, i.value)
        })
        .collect()
}

/// The adapter part of an instance name, `luid_0x…_0x…_phys_0`, and for an engine also which
/// engine. Engine instances look like `pid_4_luid_0x0_0xD1B5_phys_0_eng_3_engtype_Copy`.
fn adapter_of(instance: &str) -> Option<&str> {
    let start = instance.find("luid_")?;
    let rest = &instance[start..];
    Some(rest.find("_eng_").map_or(rest, |end| &rest[..end]))
}

fn engine_of(instance: &str) -> Option<&str> {
    let start = instance.find("luid_")?;
    let rest = &instance[start..];
    Some(rest.find("_engtype_").map_or(rest, |end| &rest[..end]))
}

/// One entry per adapter, in a stable order.
fn summarise(engines: &[(String, f64)], memory: &[(String, f64)]) -> Vec<GpuStatus> {
    let mut per_engine: BTreeMap<&str, f64> = BTreeMap::new();
    for (instance, value) in engines {
        if let Some(engine) = engine_of(instance) {
            *per_engine.entry(engine).or_default() += value.max(0.0);
        }
    }
    let mut adapters: BTreeMap<&str, GpuStatus> = BTreeMap::new();
    for (engine, busy) in per_engine {
        let Some(adapter) = adapter_of(engine) else {
            continue;
        };
        let gpu = adapters.entry(adapter).or_default();
        let busy = busy.min(100.0) as f32;
        gpu.usage_percent = Some(gpu.usage_percent.map_or(busy, |b| b.max(busy)));
    }
    for (instance, bytes) in memory {
        if let Some(adapter) = adapter_of(instance) {
            adapters.entry(adapter).or_default().memory_used_bytes = Some(bytes.max(0.0) as u64);
        }
    }
    adapters.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_processes_per_engine_and_takes_the_busiest_engine() {
        let engines = [
            ("pid_10_luid_0x0_0xA_phys_0_eng_0_engtype_3D", 20.0),
            ("pid_11_luid_0x0_0xA_phys_0_eng_0_engtype_3D", 15.0),
            ("pid_10_luid_0x0_0xA_phys_0_eng_3_engtype_Copy", 30.0),
            ("pid_12_luid_0x0_0xB_phys_0_eng_0_engtype_3D", 250.0),
        ]
        .map(|(n, v)| (n.to_string(), v));
        let memory = [("luid_0x0_0xA_phys_0", 2048.0)].map(|(n, v)| (n.to_string(), v));
        let gpus = summarise(&engines, &memory);
        assert_eq!(gpus.len(), 2);
        assert_eq!(gpus[0].usage_percent, Some(35.0));
        assert_eq!(gpus[0].memory_used_bytes, Some(2048));
        assert_eq!(gpus[1].usage_percent, Some(100.0), "capped at 100");
        assert_eq!(gpus[1].memory_used_bytes, None);
        assert!(gpus.iter().all(|g| g.name.is_none()));
    }

    #[test]
    fn no_counters_is_no_gpus() {
        assert!(summarise(&[], &[]).is_empty());
    }

    #[test]
    fn reading_the_real_counters_never_fails() {
        // A CI machine may have no GPU counters at all; either way this must return.
        for gpu in read() {
            assert!(gpu
                .usage_percent
                .map_or(true, |p| (0.0..=100.0).contains(&p)));
        }
    }
}
