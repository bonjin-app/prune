//! Batteries of connected Bluetooth accessories: the charge Windows Settings shows under
//! Bluetooth & devices, read from each present device's property store through setupapi.dll.
//!
//! Windows keeps the charge in the device property `{104EA319-6EE2-4701-BD47-8DDBF425BBE5} 2`,
//! filled in by its Bluetooth stack for devices that report one (hands-free headsets, and Bluetooth
//! LE mice and keyboards through the Battery Service). Reading it needs no rights and changes
//! nothing. Devices whose maker reports the charge only to its own software are not included.

use crate::models::AccessoryBattery;

#[repr(C)]
#[derive(Clone, Copy)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

/// `DEVPROPKEY`.
#[repr(C)]
struct PropertyKey {
    fmtid: Guid,
    pid: u32,
}

/// `SP_DEVINFO_DATA`.
#[repr(C)]
struct DeviceInfo {
    size: u32,
    class_guid: Guid,
    dev_inst: u32,
    reserved: usize,
}

#[link(name = "setupapi")]
extern "system" {
    fn SetupDiGetClassDevsW(
        class_guid: *const Guid,
        enumerator: *const u16,
        parent: isize,
        flags: u32,
    ) -> isize;
    fn SetupDiEnumDeviceInfo(set: isize, index: u32, data: *mut DeviceInfo) -> i32;
    fn SetupDiGetDevicePropertyW(
        set: isize,
        data: *const DeviceInfo,
        key: *const PropertyKey,
        property_type: *mut u32,
        buffer: *mut u8,
        size: u32,
        required: *mut u32,
        flags: u32,
    ) -> i32;
    fn SetupDiDestroyDeviceInfoList(set: isize) -> i32;
}

const INVALID_HANDLE: isize = -1;
const DIGCF_PRESENT: u32 = 0x2;
const DIGCF_ALLCLASSES: u32 = 0x4;
const DEVPROP_TYPE_BYTE: u32 = 0x3;
const DEVPROP_TYPE_STRING: u32 = 0x12;

/// `DEVPKEY_Bluetooth_Battery`.
const BATTERY: PropertyKey = PropertyKey {
    fmtid: Guid {
        data1: 0x104E_A319,
        data2: 0x6EE2,
        data3: 0x4701,
        data4: [0xBD, 0x47, 0x8D, 0xDB, 0xF4, 0x25, 0xBB, 0xE5],
    },
    pid: 2,
};

/// `DEVPKEY_Device_FriendlyName`.
const FRIENDLY_NAME: PropertyKey = PropertyKey {
    fmtid: Guid {
        data1: 0xA45C_254E,
        data2: 0xDF1C,
        data3: 0x4EFD,
        data4: [0x80, 0x20, 0x67, 0xD1, 0x46, 0xA8, 0x50, 0xE0],
    },
    pid: 14,
};

/// `DEVPKEY_NAME`, the name shown when there is no friendly one.
const NAME: PropertyKey = PropertyKey {
    fmtid: Guid {
        data1: 0xB725_F130,
        data2: 0x47EF,
        data3: 0x101A,
        data4: [0xA5, 0xF1, 0x02, 0x60, 0x8C, 0x9E, 0xEB, 0xAC],
    },
    pid: 10,
};

/// Closes the device list however reading ends.
struct DeviceList(isize);

impl Drop for DeviceList {
    fn drop(&mut self) {
        // SAFETY: a list returned by SetupDiGetClassDevsW, destroyed exactly once.
        unsafe { SetupDiDestroyDeviceInfoList(self.0) };
    }
}

pub fn read() -> Vec<AccessoryBattery> {
    // SAFETY: null class and enumerator with DIGCF_ALLCLASSES list every present device.
    let set = unsafe {
        SetupDiGetClassDevsW(
            std::ptr::null(),
            std::ptr::null(),
            0,
            DIGCF_PRESENT | DIGCF_ALLCLASSES,
        )
    };
    if set == INVALID_HANDLE || set == 0 {
        return Vec::new();
    }
    let list = DeviceList(set);
    let mut found: Vec<AccessoryBattery> = Vec::new();
    for index in 0.. {
        let mut device = DeviceInfo {
            size: std::mem::size_of::<DeviceInfo>() as u32,
            class_guid: Guid {
                data1: 0,
                data2: 0,
                data3: 0,
                data4: [0; 8],
            },
            dev_inst: 0,
            reserved: 0,
        };
        // SAFETY: `device.size` is set as the API requires; it fails past the last device.
        if unsafe { SetupDiEnumDeviceInfo(list.0, index, &mut device) } == 0 {
            break;
        }
        let Some(percent) = byte_property(&list, &device, &BATTERY).filter(|p| *p <= 100) else {
            continue;
        };
        let Some(name) = string_property(&list, &device, &FRIENDLY_NAME)
            .or_else(|| string_property(&list, &device, &NAME))
        else {
            continue;
        };
        add(&mut found, name, percent);
    }
    found
}

/// One row per accessory: Windows can list the same one under several of its services.
fn add(found: &mut Vec<AccessoryBattery>, name: String, percent: u8) {
    let name = name.trim().to_string();
    if name.is_empty() || found.iter().any(|f| f.name == name) {
        return;
    }
    found.push(AccessoryBattery {
        name,
        part: None,
        percent: f32::from(percent),
    });
}

fn byte_property(list: &DeviceList, device: &DeviceInfo, key: &PropertyKey) -> Option<u8> {
    let mut kind = 0u32;
    let mut value = 0u8;
    let mut required = 0u32;
    // SAFETY: a one-byte buffer for a property whose type is checked before it is used.
    let ok = unsafe {
        SetupDiGetDevicePropertyW(
            list.0,
            device,
            key,
            &mut kind,
            &mut value,
            1,
            &mut required,
            0,
        )
    };
    (ok != 0 && kind == DEVPROP_TYPE_BYTE).then_some(value)
}

fn string_property(list: &DeviceList, device: &DeviceInfo, key: &PropertyKey) -> Option<String> {
    let mut kind = 0u32;
    let mut buffer = [0u16; 256];
    let mut required = 0u32;
    // SAFETY: `buffer` holds 512 bytes; a longer name fails the call and is skipped.
    let ok = unsafe {
        SetupDiGetDevicePropertyW(
            list.0,
            device,
            key,
            &mut kind,
            buffer.as_mut_ptr().cast(),
            std::mem::size_of_val(&buffer) as u32,
            &mut required,
            0,
        )
    };
    if ok == 0 || kind != DEVPROP_TYPE_STRING {
        return None;
    }
    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    Some(String::from_utf16_lossy(&buffer[..len]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_row_per_accessory() {
        let mut found = Vec::new();
        add(&mut found, "Surface Headphones".into(), 80);
        add(&mut found, " Surface Headphones ".into(), 80);
        add(&mut found, "".into(), 10);
        add(&mut found, "MX Keys".into(), 40);
        let names: Vec<&str> = found.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["Surface Headphones", "MX Keys"]);
    }

    #[test]
    fn reading_this_machine_never_fails() {
        for b in read() {
            assert!((0.0..=100.0).contains(&b.percent));
        }
    }
}
