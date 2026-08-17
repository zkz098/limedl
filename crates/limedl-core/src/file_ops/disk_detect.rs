// ── Disk Detection ────────────────────────────────────

#[cfg(windows)]
mod imp {
    use std::collections::HashMap;
    use std::ffi::OsStr;
    use std::mem;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use std::sync::OnceLock;

    use parking_lot::Mutex;

    use crate::types::DiskType;

    // IOCTL code for STORAGE_QUERY_PROPERTY
    const IOCTL_STORAGE_QUERY_PROPERTY: u32 = 0x002D1400;
    const STORAGE_DEVICE_SEEK_PENALTY_PROPERTY: u32 = 7;
    const FILE_READ_ATTRIBUTES: u32 = 0x0080;
    const PROPERTY_STANDARD_QUERY: u32 = 0;

    #[repr(C)]
    struct StoragePropertyQuery {
        property_id: u32,
        query_type: u32,
        additional_parameters: [u8; 1],
    }

    #[repr(C)]
    struct StorageDeviceSeekPenaltyDescriptor {
        version: u32,
        size: u32,
        incurs_seek_penalty: u8, // BOOLEAN: 0 = SSD, 1 = HDD
        _reserved: [u8; 3],
    }

    #[repr(C)]
    struct SecurityAttributes {
        n_length: u32,
        lp_security_descriptor: *mut std::ffi::c_void,
        b_inherit_handle: i32,
    }

    // Win32 FFI declarations
    unsafe extern "system" {
        fn CreateFileW(
            lp_file_name: *const u16,
            dw_desired_access: u32,
            dw_share_mode: u32,
            lp_security_attributes: *const SecurityAttributes,
            dw_creation_disposition: u32,
            dw_flags_and_attributes: u32,
            h_template_file: *mut std::ffi::c_void,
        ) -> isize; // HANDLE

        fn DeviceIoControl(
            h_device: isize,
            dw_io_control_code: u32,
            lp_in_buffer: *const std::ffi::c_void,
            n_in_buffer_size: u32,
            lp_out_buffer: *mut std::ffi::c_void,
            n_out_buffer_size: u32,
            lp_bytes_returned: *mut u32,
            lp_overlapped: *mut std::ffi::c_void,
        ) -> i32; // BOOL

        fn CloseHandle(h_object: isize) -> i32;
        fn GetLastError() -> u32;
    }

    // Drive enumeration
    unsafe extern "system" {
        fn GetLogicalDrives() -> u32;
        fn GetDriveTypeW(lp_root_path_name: *const u16) -> u32;
    }

    const DRIVE_FIXED: u32 = 3;
    const DRIVE_REMOVABLE: u32 = 2;

    const INVALID_HANDLE_VALUE: isize = -1;
    const FILE_SHARE_READ: u32 = 1;
    const FILE_SHARE_WRITE: u32 = 2;
    const OPEN_EXISTING: u32 = 3;

    /// Convert a string to a null-terminated wide string (UTF-16).
    fn to_wide_null(s: &str) -> Vec<u16> {
        OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    /// Open the volume handle for a drive letter (e.g., "C:" → `\\.\C:`).
    /// Tries dwDesiredAccess=0 first (avoids admin requirement), then retries
    /// with FILE_READ_ATTRIBUTES if zero-access is rejected by the driver stack.
    fn open_volume(drive_letter: &str) -> Option<isize> {
        let volume_path = format!("\\\\.\\{drive_letter}");
        let wide_path = to_wide_null(&volume_path);

        // Try zero-access first (avoids admin requirement)
        if let Some(handle) = try_open_volume_inner(wide_path.as_ptr(), 0, &volume_path) {
            return Some(handle);
        }
        // Retry with FILE_READ_ATTRIBUTES — some USB bridge drivers reject zero-access
        try_open_volume_inner(wide_path.as_ptr(), FILE_READ_ATTRIBUTES, &volume_path)
    }

    /// Inner helper that opens a volume handle with a specific access mask.
    fn try_open_volume_inner(
        wide_path: *const u16,
        access: u32,
        volume_path: &str,
    ) -> Option<isize> {
        let handle = unsafe {
            CreateFileW(
                wide_path,
                access,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };

        if handle == INVALID_HANDLE_VALUE {
            let err = unsafe { GetLastError() };
            tracing::debug!(
                "disk_detect: CreateFileW({volume_path}) with access={access} failed, error={err}"
            );
            None
        } else {
            tracing::trace!("disk_detect: opened {volume_path} with access={access}");
            Some(handle)
        }
    }

    /// Process-lifetime cache of drive-letter → disk type.
    ///
    /// The seek-penalty property of a mounted volume is stable for the lifetime
    /// of the process, so re-querying via `DeviceIoControl` on every download
    /// start (`resolve_disk_type`) or settings-panel scan is wasted syscalls and
    /// log spam — some volume stacks (virtual disks, certain USB bridges) don't
    /// support the property and emit a warning on every call. Only successfully
    /// opened volumes are cached: an unopenable drive is NOT cached so a later
    /// mount is re-detected.
    fn disk_type_cache() -> &'static Mutex<HashMap<String, DiskType>> {
        static CACHE: OnceLock<Mutex<HashMap<String, DiskType>>> = OnceLock::new();
        CACHE.get_or_init(|| Mutex::new(HashMap::new()))
    }

    pub fn detect_disk_type(path: &Path) -> DiskType {
        // Get the drive letter or mount point root
        let drive_letter = match path.components().next() {
            Some(std::path::Component::Prefix(prefix)) => {
                match prefix.kind() {
                    std::path::Prefix::Disk(byte) | std::path::Prefix::VerbatimDisk(byte) => {
                        format!("{}:", byte as char)
                    }
                    _ => {
                        tracing::warn!("disk_detect: unsupported path prefix, defaulting to SSD");
                        return DiskType::Ssd; // UNC paths, device namespace — fallback
                    }
                }
            }
            Some(std::path::Component::RootDir) => {
                // Relative root — use the current drive
                match std::env::current_dir() {
                    Ok(cwd) => match cwd.components().next() {
                        Some(std::path::Component::Prefix(p)) => match p.kind() {
                            std::path::Prefix::Disk(byte)
                            | std::path::Prefix::VerbatimDisk(byte) => {
                                format!("{}:", byte as char)
                            }
                            _ => return DiskType::Ssd,
                        },
                        _ => return DiskType::Ssd,
                    },
                    Err(_) => return DiskType::Ssd,
                }
            }
            _ => return DiskType::Ssd,
        };

        // Fast path: this volume was already detected this session.
        if let Some(&cached) = disk_type_cache().lock().get(&drive_letter) {
            return cached;
        }

        let handle = match open_volume(&drive_letter) {
            Some(h) => h,
            None => {
                tracing::warn!(
                    "disk_detect: could not open volume for {drive_letter}, defaulting to SSD"
                );
                return DiskType::Ssd;
            }
        };

        // Query the seek penalty property (documented, reliable — available since Windows 7).
        // IncursSeekPenalty=1 → rotating HDD, 0 → non-rotating SSD.
        let result = query_seek_penalty(handle);
        unsafe { CloseHandle(handle); }

        // Cache the result (including the SSD fallback) — stable for a mounted
        // volume within this process.
        disk_type_cache().lock().insert(drive_letter.clone(), result);

        match result {
            DiskType::Hdd => tracing::debug!("disk_detect: {drive_letter} is HDD (seek penalty)"),
            DiskType::Ssd => tracing::debug!("disk_detect: {drive_letter} is SSD (no seek penalty)"),
        }
        result
    }

    fn query_seek_penalty(handle: isize) -> DiskType {
        let query = StoragePropertyQuery {
            property_id: STORAGE_DEVICE_SEEK_PENALTY_PROPERTY,
            query_type: PROPERTY_STANDARD_QUERY,
            additional_parameters: [0; 1],
        };

        let mut descriptor: StorageDeviceSeekPenaltyDescriptor = unsafe { mem::zeroed() };
        let mut bytes_returned: u32 = 0;

        let success = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_STORAGE_QUERY_PROPERTY,
                &query as *const _ as *const std::ffi::c_void,
                mem::size_of::<StoragePropertyQuery>() as u32,
                &mut descriptor as *mut _ as *mut std::ffi::c_void,
                mem::size_of::<StorageDeviceSeekPenaltyDescriptor>() as u32,
                &mut bytes_returned,
                std::ptr::null_mut(),
            )
        };

        if success == 0 {
            let err = unsafe { GetLastError() };
            tracing::warn!(
                "disk_detect: DeviceIoControl(STORAGE_SEEK_PENALTY) failed, error={err}, defaulting to SSD"
            );
            return DiskType::Ssd;
        }
        if bytes_returned == 0 {
            tracing::warn!(
                "disk_detect: DeviceIoControl(STORAGE_SEEK_PENALTY) returned 0 bytes, defaulting to SSD"
            );
            return DiskType::Ssd;
        }

        if descriptor.incurs_seek_penalty == 0 {
            DiskType::Ssd
        } else {
            DiskType::Hdd
        }
    }

    /// Enumerate all fixed/removable drives and detect disk type for each.
    pub fn detect_all_disk_types() -> HashMap<String, DiskType> {
        let drives_mask = unsafe { GetLogicalDrives() };
        let mut result = HashMap::new();
        for i in 0..26u32 {
            if drives_mask & (1 << i) != 0 {
                let letter = (b'A' + i as u8) as char;
                let drive_root = format!("{letter}:\\");
                let root_wide = to_wide_null(&drive_root);
                let drive_type = unsafe { GetDriveTypeW(root_wide.as_ptr()) };
                if drive_type == DRIVE_FIXED || drive_type == DRIVE_REMOVABLE {
                    let disk_type = detect_disk_type(Path::new(&drive_root));
                    result.insert(format!("{letter}:"), disk_type);
                }
            }
        }
        result
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use std::fs;
    use std::path::Path;

    use crate::types::DiskType;

    pub fn detect_disk_type(path: &Path) -> DiskType {
        let Ok(meta) = fs::metadata(path) else {
            return DiskType::Ssd;
        };
        use std::os::linux::fs::MetadataExt;
        let dev = meta.st_dev();
        let major = libc::major(dev);
        let minor = libc::minor(dev);

        let dev_symlink = format!("/sys/dev/block/{major}:{minor}");
        let Ok(link) = fs::read_link(&dev_symlink) else {
            return DiskType::Ssd;
        };

        // Use file_name() directly — handles both partition (sda1) and
        // whole-disk (sda) cases, since modern Linux creates sysfs entries
        // for partition devices under /sys/block/ as well.
        let Some(device_name) = link
            .file_name()
            .and_then(|n| n.to_str())
        else {
            return DiskType::Ssd;
        };

        let rotational_path = format!("/sys/block/{device_name}/queue/rotational");
        match fs::read_to_string(&rotational_path) {
            Ok(val) if val.trim() == "1" => DiskType::Hdd,
            _ => DiskType::Ssd,
        }
    }

    use std::collections::HashMap;

    pub fn detect_all_disk_types() -> HashMap<String, DiskType> {
        let mut result = HashMap::new();
        let Ok(entries) = fs::read_dir("/sys/block") else {
            return result;
        };
        for entry in entries.flatten() {
            let rotational_path = entry.path().join("queue/rotational");
            if let Ok(val) = fs::read_to_string(&rotational_path) {
                let name = entry.file_name().to_string_lossy().to_string();
                let disk_type = if val.trim() == "1" { DiskType::Hdd } else { DiskType::Ssd };
                result.insert(name, disk_type);
            }
        }
        result
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use std::ffi::{c_void, CStr};
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

    pub fn detect_disk_type(path: &Path) -> DiskType {
        // 1. Get the BSD device name via statfs
        let abs_path = match std::fs::canonicalize(path) {
            Ok(p) => p,
            Err(_) => return DiskType::Ssd,
        };

        let path_c = match std::ffi::CString::new(abs_path.to_string_lossy().as_bytes()) {
            Ok(c) => c,
            Err(_) => return DiskType::Ssd,
        };

        let mut fsbuf: libc::statfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statfs(path_c.as_ptr(), &mut fsbuf) } != 0 {
            return DiskType::Ssd;
        }

        let mntfrom = unsafe { CStr::from_ptr(fsbuf.f_mntfromname.as_ptr()) };
        let dev_path_str = mntfrom.to_string_lossy();

        let bsd_full = match std::path::Path::new(dev_path_str.as_ref()).file_name() {
            Some(n) => n.to_string_lossy().to_string(),
            None => return DiskType::Ssd,
        };

        let target_disk = base_disk_name(&bsd_full);

        // 2. Query IOKit IOMedia for this specific disk
        let matching =
            unsafe { IOServiceMatching(c"IOMedia".as_ptr()) };
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

            // Check if this IOMedia entry matches our target disk
            let bsd_prop = unsafe {
                IORegistryEntryCreateCFProperty(entry, bsd_name_key, std::ptr::null(), 0)
            };
            if bsd_prop.is_null() {
                unsafe { IOObjectRelease(entry) };
                continue;
            }

            let matches = unsafe {
                cfstring_to_string(bsd_prop as CFStringRef)
                    .is_some_and(|name| name == target_disk || name.starts_with(&format!("{target_disk}s")))
            };
            unsafe { CFRelease(bsd_prop) };

            if !matches {
                unsafe { IOObjectRelease(entry) };
                continue;
            }

            // Walk up to IOBlockStorageDriver and check Rotational
            let mut current = entry;
            let plane = c"IOService".as_ptr();
            let Some(rot_key) = make_cfstr("Rotational") else {
                break;
            };

            for depth in 0..8 {
                let prop = unsafe {
                    IORegistryEntryCreateCFProperty(current, rot_key, std::ptr::null(), 0)
                };

                if !prop.is_null()
                    && let Some(is_rotational) = unsafe { cfbool_value(prop) }
                {
                    result = if is_rotational {
                        DiskType::Hdd
                    } else {
                        DiskType::Ssd
                    };
                    if depth > 0 {
                        unsafe { IOObjectRelease(current) };
                    }
                    break;
                }

                let mut parent: io_registry_entry_t = 0;
                let kr =
                    unsafe { IORegistryEntryGetParentEntry(current, plane, &mut parent) };
                if depth > 0 {
                    unsafe { IOObjectRelease(current) };
                }
                if kr != 0 || parent == 0 {
                    break;
                }
                current = parent;
            }

            unsafe { CFRelease(rot_key as CFTypeRef) };
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
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
mod imp {
    use std::path::Path;

    use crate::types::DiskType;

    // NOTE: This fallback is for platforms without a native disk detection
    // backend (not Windows/Linux/macOS). Linux and macOS have their own
    // implementions above; this module only covers unknown targets.
    pub fn detect_disk_type(_path: &Path) -> DiskType {
        DiskType::Ssd
    }

    use std::collections::HashMap;

    pub fn detect_all_disk_types() -> HashMap<String, DiskType> {
        HashMap::new()
    }
}

pub use imp::detect_disk_type;
pub use imp::detect_all_disk_types;
