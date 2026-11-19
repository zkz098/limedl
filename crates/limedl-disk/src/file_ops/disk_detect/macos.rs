use std::ffi::{CStr, c_void};
use std::path::Path;

use crate::types::DiskType;

#[allow(non_camel_case_types)]
type io_object_t = u32;
#[allow(non_camel_case_types)]
type io_iterator_t = io_object_t;
#[allow(non_camel_case_types)]
type io_registry_entry_t = io_object_t;
#[allow(non_camel_case_types)]
type kern_return_t = i32;
type CFStringRef = *const c_void;
type CFBooleanRef = *const c_void;
type CFMutableDictionaryRef = *const c_void;
type CFTypeRef = *const c_void;
type CFAllocatorRef = *const c_void;

#[allow(clippy::duplicated_attributes)]
#[link(name = "CoreFoundation", kind = "framework")]
#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOServiceMatching(name: *const i8) -> CFMutableDictionaryRef;
    fn IOServiceGetMatchingServices(
        mainPort: u32,
        matching: CFMutableDictionaryRef,
        existing: *mut io_iterator_t,
    ) -> kern_return_t;
    fn IOIteratorNext(iterator: io_iterator_t) -> io_object_t;
    fn IOObjectRelease(object: io_object_t) -> kern_return_t;
    fn IORegistryEntryCreateCFProperty(
        entry: io_registry_entry_t,
        key: CFStringRef,
        allocator: CFAllocatorRef,
        options: u32,
    ) -> CFTypeRef;
    fn IORegistryEntryGetParentEntry(
        entry: io_registry_entry_t,
        plane: *const i8,
        parent: *mut io_registry_entry_t,
    ) -> kern_return_t;
    fn CFStringCreateWithCString(
        alloc: CFAllocatorRef,
        cStr: *const i8,
        encoding: u32,
    ) -> CFStringRef;
    fn CFStringGetCString(
        theString: CFStringRef,
        buffer: *mut i8,
        bufferSize: i64,
        encoding: u32,
    ) -> u8;
    fn CFRelease(cf: CFTypeRef);
    fn CFBooleanGetValue(boolean: CFBooleanRef) -> u8;
    fn CFGetTypeID(cf: CFTypeRef) -> usize;
    fn CFBooleanGetTypeID() -> usize;
}

fn make_cfstr(s: &str) -> Option<CFStringRef> {
    let c = std::ffi::CString::new(s).ok()?;
    let cf = unsafe { CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), 0x0800_0100) };
    if cf.is_null() { None } else { Some(cf) }
}

unsafe fn cfbool_value(cf: CFTypeRef) -> Option<bool> {
    if cf.is_null() {
        return None;
    }
    if unsafe { CFGetTypeID(cf) != CFBooleanGetTypeID() } {
        unsafe { CFRelease(cf) };
        return None;
    }
    let v = unsafe { CFBooleanGetValue(cf as CFBooleanRef) } != 0;
    unsafe { CFRelease(cf) };
    Some(v)
}

/// Read a CFString into a Rust String.
unsafe fn cfstring_to_string(cf: CFStringRef) -> Option<String> {
    if cf.is_null() {
        return None;
    }
    let mut buf = vec![0i8; 256];
    if unsafe { CFStringGetCString(cf, buf.as_mut_ptr(), buf.len() as i64, 0x0800_0100) } != 0 {
        let cstr = unsafe { CStr::from_ptr(buf.as_ptr()) };
        Some(cstr.to_string_lossy().to_string())
    } else {
        None
    }
}

/// Extract the base disk name from a BSD device name.
/// "disk1s2" → "disk1", "disk0" → "disk0"
fn base_disk_name(bsd: &str) -> &str {
    if let Some(s_idx) = bsd.find('s') {
        &bsd[..s_idx]
    } else {
        bsd
    }
}

/// `statfs` for the path; `None` when the path cannot be canonicalized or
/// the syscall fails.
fn statfs_for(path: &Path) -> Option<libc::statfs> {
    let abs_path = std::fs::canonicalize(path).ok()?;
    let path_c = std::ffi::CString::new(abs_path.to_string_lossy().as_bytes()).ok()?;
    let mut fsbuf: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statfs(path_c.as_ptr(), &mut fsbuf) } != 0 {
        return None;
    }
    Some(fsbuf)
}

/// BSD device name (for example `disk1s2`) backing the mount, if any.
fn mounted_bsd_name(fsbuf: &libc::statfs) -> Option<String> {
    let mntfrom = unsafe { CStr::from_ptr(fsbuf.f_mntfromname.as_ptr()) };
    let dev_path_str = mntfrom.to_string_lossy();
    std::path::Path::new(dev_path_str.as_ref())
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
}

pub fn detect_disk_type(path: &Path) -> DiskType {
    // 1. Get the BSD device name via statfs
    let Some(fsbuf) = statfs_for(path) else {
        return DiskType::Ssd;
    };

    // A network mount has no IOKit media of its own: the "Rotational" probe
    // below would either find nothing or describe unrelated hardware, so
    // classify the mount by its filesystem type instead.
    let fstype = unsafe { CStr::from_ptr(fsbuf.f_fstypename.as_ptr()) };
    if crate::file_ops::is_network_filesystem(&fstype.to_string_lossy()) {
        return DiskType::Network;
    }

    let Some(bsd_full) = mounted_bsd_name(&fsbuf) else {
        return DiskType::Ssd;
    };
    let target_disk = base_disk_name(&bsd_full);

    // 2. Query IOKit IOMedia for this specific disk
    disk_type_via_iokit(target_disk)
}

/// Query IOKit `IOMedia` entries for `target_disk` and report the first
/// `Rotational` property found while walking up to the storage driver.
fn disk_type_via_iokit(target_disk: &str) -> DiskType {
    let matching = unsafe { IOServiceMatching(c"IOMedia".as_ptr()) };
    if matching.is_null() {
        return DiskType::Ssd;
    }

    let mut iter: io_iterator_t = 0;
    let kr = unsafe { IOServiceGetMatchingServices(0, matching, &mut iter) };
    if kr != 0 {
        return DiskType::Ssd;
    }

    let mut result = DiskType::Ssd;
    let Some(bsd_name_key) = make_cfstr("BSD Name") else {
        return DiskType::Ssd;
    };

    loop {
        let entry = unsafe { IOIteratorNext(iter) };
        if entry == 0 {
            break;
        }

        if !iomedia_matches_disk(entry, bsd_name_key, target_disk) {
            unsafe { IOObjectRelease(entry) };
            continue;
        }

        // Walk up to IOBlockStorageDriver and check Rotational
        let Some(is_rotational) = rotational_of_entry(entry) else {
            // The `Rotational` key could not be created — stop scanning,
            // matching the previous behavior (the entry is left to the
            // iterator teardown).
            break;
        };
        result = if is_rotational {
            DiskType::Hdd
        } else {
            DiskType::Ssd
        };
        unsafe { IOObjectRelease(entry) };

        if result != DiskType::Ssd {
            break;
        }
    }

    unsafe {
        CFRelease(bsd_name_key as CFTypeRef);
        IOObjectRelease(iter);
    }
    result
}

/// Whether an `IOMedia` entry's `BSD Name` belongs to `target_disk`.
fn iomedia_matches_disk(
    entry: io_registry_entry_t,
    bsd_name_key: CFStringRef,
    target_disk: &str,
) -> bool {
    let bsd_prop =
        unsafe { IORegistryEntryCreateCFProperty(entry, bsd_name_key, std::ptr::null(), 0) };
    if bsd_prop.is_null() {
        return false;
    }

    let matches = unsafe {
        cfstring_to_string(bsd_prop as CFStringRef)
            .is_some_and(|name| name == target_disk || name.starts_with(&format!("{target_disk}s")))
    };
    unsafe { CFRelease(bsd_prop) };
    matches
}

/// Walk up from `entry` looking for the `Rotational` property.
///
/// `None` means the `Rotational` key could not be created; the caller stops
/// scanning and leaves `entry` to the iterator teardown (historical
/// behavior).
fn rotational_of_entry(entry: io_registry_entry_t) -> Option<bool> {
    let mut current = entry;
    let plane = c"IOService".as_ptr();
    let rot_key = make_cfstr("Rotational")?;
    let mut result = None;

    for depth in 0..8 {
        let prop =
            unsafe { IORegistryEntryCreateCFProperty(current, rot_key, std::ptr::null(), 0) };

        if !prop.is_null()
            && let Some(is_rotational) = unsafe { cfbool_value(prop) }
        {
            result = Some(is_rotational);
            if depth > 0 {
                unsafe { IOObjectRelease(current) };
            }
            break;
        }

        let mut parent: io_registry_entry_t = 0;
        let kr = unsafe { IORegistryEntryGetParentEntry(current, plane, &mut parent) };
        if depth > 0 {
            unsafe { IOObjectRelease(current) };
        }
        if kr != 0 || parent == 0 {
            break;
        }
        current = parent;
    }

    unsafe { CFRelease(rot_key as CFTypeRef) };
    result
}

use std::collections::HashMap;

pub fn detect_all_disk_types() -> HashMap<String, DiskType> {
    // Scan the root volume at minimum — avoids regression from the old
    // single-path detect_disk_type which worked correctly for macOS.
    // Full IOKit enumeration of all IOMedia entries is TODO.
    let mut result = HashMap::new();
    let root = Path::new("/");
    let disk_type = detect_disk_type(root);
    result.insert("root".to_string(), disk_type);
    result
}
