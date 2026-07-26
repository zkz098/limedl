// ── File Allocation ──────────────────────────────────

use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, BufReader, ErrorKind, Read, Write},
    path::Path,
};

use fs4::FileExt;

use tracing;

use super::error::{DownloadError, Result, io_error_with_path};

pub fn open_download_file(path: &Path, total_size: Option<u64>) -> Result<File> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| io_error_with_path(e, parent.to_string_lossy()))?;
    }

    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| io_error_with_path(e, path.to_string_lossy()))?;

    preallocate_file(&file, total_size)?;
    Ok(file)
}

pub fn reset_download_file(file: &File, total_size: Option<u64>) -> Result<()> {
    file.set_len(0)?;
    preallocate_file(file, total_size)
}

pub fn finalize_temp_file(temp_path: &Path, destination_path: &Path) -> Result<()> {
    if destination_path.exists() {
        if files_have_same_content(temp_path, destination_path)? {
            cleanup_finalizing_paths(destination_path)?;
            fs::remove_file(temp_path)?;
            return Ok(());
        }
        return Err(destination_exists_error(destination_path));
    }

    // Primary path: atomic rename (works on the same filesystem).
    // rename is a metadata-only operation — no data copy, no sync_all needed.
    // On same-volume moves this is O(1) and atomic on both POSIX and Windows.
    match std::fs::rename(temp_path, destination_path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::CrossesDevices => {
            // Fallback: copy via staging path when source and destination
            // reside on different mount points / drive letters.
            let staging_path = unique_finalizing_path(destination_path)?;
            let mut source = File::open(temp_path)?;
            let mut destination = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&staging_path)
                .map_err(DownloadError::Io)?;

            if let Err(error) = copy_file_buffered(&mut source, &mut destination) {
                drop(destination);
                drop(source);
                if let Err(e) = fs::remove_file(&staging_path) {
                    tracing::warn!(
                        "Failed to clean up staging file {}: {e}",
                        staging_path.display()
                    );
                }
                return Err(error.into());
            }
            destination.flush()?;
            destination.sync_all()?;
            drop(destination);
            drop(source);

            if let Err(error) = fs::hard_link(&staging_path, destination_path) {
                if error.kind() == ErrorKind::AlreadyExists
                    && files_have_same_content(&staging_path, destination_path)?
                {
                    fs::remove_file(&staging_path)
                        .map_err(|e| io_error_with_path(e, staging_path.to_string_lossy()))?;
                    fs::remove_file(temp_path)
                        .map_err(|e| io_error_with_path(e, temp_path.to_string_lossy()))?;
                    return Ok(());
                }
                if error.kind() == ErrorKind::AlreadyExists {
                    let _ = fs::remove_file(&staging_path);
                    return Err(destination_exists_error(destination_path));
                }

                fallback_copy_staging_to_destination(&staging_path, destination_path)?;
            }

            fs::remove_file(&staging_path)
                .map_err(|e| io_error_with_path(e, staging_path.to_string_lossy()))?;
            fs::remove_file(temp_path)
                .map_err(|e| io_error_with_path(e, temp_path.to_string_lossy()))?;
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

fn fallback_copy_staging_to_destination(
    staging_path: &Path,
    destination_path: &Path,
) -> Result<()> {
    let mut source = File::open(staging_path)?;
    let mut destination = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination_path)
        .map_err(|error| {
            if error.kind() == ErrorKind::AlreadyExists {
                destination_exists_error(destination_path)
            } else {
                DownloadError::Io(error)
            }
        })?;

    if let Err(error) = copy_file_buffered(&mut source, &mut destination) {
        drop(destination);
        drop(source);
        let _ = fs::remove_file(destination_path);
        return Err(error.into());
    }
    destination.flush()?;
    destination.sync_all()?;
    Ok(())
}

/// Copy file contents using a 256 KB stack-allocated buffer instead of the
/// stdlib default 8 KB buffer used by [`io::copy`].
fn copy_file_buffered(source: &mut File, dest: &mut File) -> io::Result<u64> {
    let mut buffer = [0u8; 262144];
    let mut total = 0u64;
    loop {
        let bytes_read = match source.read(&mut buffer) {
            Ok(0) => return Ok(total),
            Ok(n) => n,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        {
            let mut buf = &buffer[..bytes_read];
            while !buf.is_empty() {
                match dest.write(buf) {
                    Ok(0) => {
                        return Err(io::Error::new(ErrorKind::WriteZero, "write returned zero"));
                    }
                    Ok(n) => buf = &buf[n..],
                    Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                }
            }
        }
        total += bytes_read as u64;
    }
}

fn destination_exists_error(destination_path: &Path) -> DownloadError {
    DownloadError::Io(io::Error::new(
        ErrorKind::AlreadyExists,
        format!(
            "destination file already exists: {}",
            destination_path.display()
        ),
    ))
}

fn files_have_same_content(left_path: &Path, right_path: &Path) -> Result<bool> {
    let left = File::open(left_path)?;
    let right = File::open(right_path)?;
    if left.metadata()?.len() != right.metadata()?.len() {
        return Ok(false);
    }

    let mut left = BufReader::new(left);
    let mut right = BufReader::new(right);
    let mut left_buffer = [0; 8192];
    let mut right_buffer = [0; 8192];
    loop {
        let left_read = left.read(&mut left_buffer)?;
        let right_read = right.read(&mut right_buffer)?;
        if left_read != right_read {
            return Ok(false);
        }
        if left_read == 0 {
            return Ok(true);
        }
        if left_buffer[..left_read] != right_buffer[..right_read] {
            return Ok(false);
        }
    }
}

fn cleanup_finalizing_paths(destination_path: &Path) -> Result<()> {
    let Some(parent) = destination_path.parent() else {
        return Ok(());
    };
    let Some(file_name) = destination_path.file_name() else {
        return Ok(());
    };
    let prefix = {
        let mut value = OsString::from(file_name);
        value.push(".finalizing.");
        value
    };

    for entry in
        fs::read_dir(parent).map_err(|e| io_error_with_path(e, parent.to_string_lossy()))?
    {
        let entry = entry.map_err(|e| io_error_with_path(e, parent.to_string_lossy()))?;
        let candidate_name = entry.file_name();
        if candidate_name
            .to_string_lossy()
            .starts_with(&prefix.to_string_lossy().to_string())
            && let Err(e) = fs::remove_file(entry.path())
        {
            tracing::warn!(
                "Failed to clean up finalizing file {}: {e}",
                entry.path().display()
            );
        }
    }
    Ok(())
}

fn unique_finalizing_path(destination_path: &Path) -> Result<std::path::PathBuf> {
    let parent = destination_path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = destination_path.file_name().ok_or_else(|| {
        DownloadError::InvalidResponse(String::from("missing destination file name"))
    })?;
    let process_id = std::process::id();

    for attempt in 0..1000u16 {
        let mut staging_name = OsString::from(file_name);
        staging_name.push(format!(".finalizing.{process_id}.{attempt}.tmp"));
        let candidate = parent.join(staging_name);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(DownloadError::Io(io::Error::new(
        ErrorKind::AlreadyExists,
        format!(
            "unable to allocate finalizing path for {}",
            destination_path.display()
        ),
    )))
}

pub fn write_all_at(file: &File, mut buffer: &[u8], mut offset: u64) -> Result<()> {
    while !buffer.is_empty() {
        let written = write_once_at(file, buffer, offset)?;
        if written == 0 {
            return Err(DownloadError::InvalidResponse(String::from(
                "failed to write download data",
            )));
        }
        offset += written as u64;
        buffer = &buffer[written..];
    }
    Ok(())
}

/// Checks that the destination directory has enough free space for the download.
/// `required_bytes` is the total file size. We require 10% buffer above that.
pub fn check_disk_space(destination_dir: &Path, required_bytes: u64) -> Result<()> {
    let available = fs4::available_space(destination_dir)?;
    let required = required_bytes + required_bytes / 10; // 10% buffer
    if available < required {
        return Err(DownloadError::InsufficientDiskSpace {
            available,
            required,
        });
    }
    Ok(())
}

fn preallocate_file(file: &File, total_size: Option<u64>) -> Result<()> {
    let Some(total_size) = total_size else {
        return Ok(());
    };

    match file.allocate(total_size) {
        Ok(()) => Ok(()),
        Err(error) => match error.raw_os_error() {
            Some(38 | 45 | 95 | 524) => {
                file.set_len(total_size)?;
                Ok(())
            }
            _ => Err(error.into()),
        },
    }
}

#[cfg(unix)]
fn write_once_at(file: &File, buffer: &[u8], offset: u64) -> std::io::Result<usize> {
    use std::os::unix::fs::FileExt;

    file.write_at(buffer, offset)
}

#[cfg(windows)]
fn write_once_at(file: &File, buffer: &[u8], offset: u64) -> std::io::Result<usize> {
    use std::os::windows::fs::FileExt;

    file.seek_write(buffer, offset)
}

// ── Disk Detection ────────────────────────────────────

use super::types::DiskType;

#[cfg(windows)]
mod imp {
    use std::ffi::OsStr;
    use std::mem;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use super::DiskType;

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

        if success == 0 || bytes_returned == 0 {
            return DiskType::Ssd;
        }

        if descriptor.incurs_seek_penalty == 0 {
            DiskType::Ssd
        } else {
            DiskType::Hdd
        }
    }

}

#[cfg(target_os = "linux")]
mod imp {
    use std::fs;
    use std::path::Path;

    use super::DiskType;

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
}

#[cfg(target_os = "macos")]
mod imp {
    use std::ffi::{c_void, CStr};
    use std::path::Path;

    use super::DiskType;

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
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
mod imp {
    use std::path::Path;

    use super::DiskType;

    // NOTE: Non-Windows platforms currently lack a cross-platform equivalent of
    // IOCTL_STORAGE_QUERY_PROPERTY. This means HDD detection always returns Ssd
    // on macOS/Linux, causing the UI to auto-disable HDD buffering. Users with
    // external HDDs should use disk_type_overrides to force HDD treatment.
    // TODO: add sysfs (Linux) / IOKit (macOS) disk type probes.
    pub fn detect_disk_type(_path: &Path) -> DiskType {
        DiskType::Ssd
    }
}

pub use imp::detect_disk_type;

// ── Tests ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use ntest::timeout;
    use tempfile::tempdir;

    use super::{
        check_disk_space, cleanup_finalizing_paths, fallback_copy_staging_to_destination,
        files_have_same_content, finalize_temp_file, open_download_file, preallocate_file,
        reset_download_file, unique_finalizing_path, write_all_at,
    };
    use crate::error::DownloadError;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

    #[timeout(30_000)]
    #[test]
    fn open_download_file_sizes_known_length() -> TestResult {
        let temp = tempdir()?;
        let path = temp.path().join("download.part");

        let file = open_download_file(&path, Some(1024 * 1024))?;

        assert_eq!(file.metadata()?.len(), 1024 * 1024);
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn reset_download_file_restores_target_length() -> TestResult {
        let temp = tempdir()?;
        let path = temp.path().join("download.part");
        let file = open_download_file(&path, Some(4096))?;

        file.set_len(128)?;
        reset_download_file(&file, Some(8192))?;

        assert_eq!(file.metadata()?.len(), 8192);
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn write_after_preallocation_preserves_file() -> TestResult {
        let temp = tempdir()?;
        let path = temp.path().join("download.part");
        let file = open_download_file(&path, Some(4096))?;

        write_all_at(&file, b"test data", 0)?;

        assert_eq!(file.metadata()?.len(), 4096);
        let bytes = fs::read(path)?;
        assert_eq!(&bytes[..9], b"test data");
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn finalize_temp_file_refuses_to_overwrite_destination() -> TestResult {
        let temp = tempdir()?;
        let source = temp.path().join("download.part");
        let destination = temp.path().join("download.bin");
        fs::write(&source, b"new")?;
        fs::write(&destination, b"existing")?;

        let result = finalize_temp_file(&source, &destination);

        assert!(result.is_err());
        assert_eq!(fs::read(&destination)?, b"existing");
        assert_eq!(fs::read(&source)?, b"new");
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn finalize_temp_file_moves_completed_download() -> TestResult {
        let temp = tempdir()?;
        let source = temp.path().join("download.part");
        let destination = temp.path().join("download.bin");
        fs::write(&source, b"complete")?;

        finalize_temp_file(&source, &destination)?;

        assert!(!source.exists());
        assert_eq!(fs::read(&destination)?, b"complete");
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn finalize_temp_file_accepts_existing_identical_destination() -> TestResult {
        let temp = tempdir()?;
        let source = temp.path().join("download.part");
        let destination = temp.path().join("download.bin");
        fs::write(&source, b"complete")?;
        fs::write(&destination, b"complete")?;

        finalize_temp_file(&source, &destination)?;

        assert!(!source.exists());
        assert_eq!(fs::read(&destination)?, b"complete");
        Ok(())
    }

    // ── check_disk_space ──────────────────────────────

    #[timeout(30_000)]
    #[test]
    fn check_disk_space_insufficient() -> TestResult {
        let temp = tempdir()?;
        let available = fs4::available_space(temp.path())?;

        // Request more than available (incl. 10% buffer) → error
        let required_bytes = available + 1;
        let result = check_disk_space(temp.path(), required_bytes);

        assert!(result.is_err());
        match result.unwrap_err() {
            DownloadError::InsufficientDiskSpace { available: a, required: r } => {
                // Disk free space may fluctuate between query and check;
                // verify the invariant rather than exact values.
                assert!(
                    a < r,
                    "available ({a}) must be less than required ({r})"
                );
                assert_eq!(
                    r,
                    required_bytes + required_bytes / 10,
                    "required should include 10% buffer"
                );
            }
            other => panic!("expected InsufficientDiskSpace, got {other:?}"),
        }
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn check_disk_space_sufficient() -> TestResult {
        let temp = tempdir()?;

        // 1 byte required — way less than any real filesystem has available
        let result = check_disk_space(temp.path(), 1);

        assert!(result.is_ok());
        Ok(())
    }

    // ── preallocate_file ──────────────────────────────

    #[timeout(30_000)]
    #[test]
    fn preallocate_file_skips_on_none() -> TestResult {
        let temp = tempdir()?;
        let path = temp.path().join("none.dat");
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)?;

        assert!(preallocate_file(&file, None).is_ok());
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn preallocate_file_fails_on_readonly_fd() -> TestResult {
        let temp = tempdir()?;
        let path = temp.path().join("readonly.dat");
        // Create writable, close, reopen read-only
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)?;
        drop(file);

        let file = fs::OpenOptions::new()
            .read(true)
            .open(&path)?;

        let result = preallocate_file(&file, Some(4096));
        assert!(result.is_err());
        Ok(())
    }

    // ── write_all_at ──────────────────────────────────

    #[timeout(30_000)]
    #[test]
    fn write_all_at_empty_buffer_succeeds() -> TestResult {
        let temp = tempdir()?;
        let path = temp.path().join("empty.dat");
        let file = open_download_file(&path, Some(1024))?;

        write_all_at(&file, b"", 0)?;
        // File should remain at the preallocated size
        assert_eq!(file.metadata()?.len(), 1024);
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn write_all_at_fails_on_readonly_fd() -> TestResult {
        let temp = tempdir()?;
        let path = temp.path().join("write_ro.dat");
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)?;
        drop(file);

        let file = fs::OpenOptions::new()
            .read(true)
            .open(&path)?;

        let result = write_all_at(&file, b"data", 0);
        assert!(result.is_err());
        Ok(())
    }

    // ── finalize_temp_file cross-device ───────────────

    #[timeout(30_000)]
    #[test]
    fn finalize_temp_file_cross_device_copy() -> TestResult {
        // Find a second writable volume to trigger CrossesDevices fallback
        let drives: Vec<String> = (b'C'..=b'Z')
            .map(|c| format!("{}:\\", c as char))
            .filter(|p| Path::new(p).exists())
            .collect();

        if drives.len() < 2 {
            // Single-drive system — can't test cross-device rename
            return Ok(());
        }

        let dir_a = tempfile::tempdir_in(&drives[0])?;
        let dir_b = tempfile::tempdir_in(&drives[1])?;

        let source = dir_a.path().join("source.dat");
        let destination = dir_b.path().join("dest.dat");
        fs::write(&source, b"cross-device content")?;

        finalize_temp_file(&source, &destination)?;

        assert!(!source.exists(), "source should be removed");
        assert_eq!(fs::read(&destination)?, b"cross-device content");
        Ok(())
    }

    // ── files_have_same_content ───────────────────────

    #[timeout(30_000)]
    #[test]
    fn files_have_same_content_different_sizes() -> TestResult {
        let temp = tempdir()?;
        let a = temp.path().join("a.txt");
        let b = temp.path().join("b.txt");
        fs::write(&a, b"short")?;
        fs::write(&b, b"longer content")?;

        assert!(!files_have_same_content(&a, &b)?);
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn files_have_same_content_same_size_different() -> TestResult {
        let temp = tempdir()?;
        let left = temp.path().join("left.bin");
        let right = temp.path().join("right.bin");
        fs::write(&left, b"AAAA")?;
        fs::write(&right, b"BBBB")?;

        assert!(!files_have_same_content(&left, &right)?);
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn files_have_same_content_identical() -> TestResult {
        let temp = tempdir()?;
        let a = temp.path().join("a.txt");
        let b = temp.path().join("b.txt");
        let content = b"The quick brown fox jumps over the lazy dog 1234567890";
        fs::write(&a, content)?;
        fs::write(&b, content)?;

        assert!(files_have_same_content(&a, &b)?);
        Ok(())
    }

    // ── unique_finalizing_path ────────────────────────

    #[timeout(30_000)]
    #[test]
    fn unique_finalizing_path_no_existing_files() -> TestResult {
        let temp = tempdir()?;
        let dest = temp.path().join("myfile.bin");
        let staging = unique_finalizing_path(&dest)?;

        let display = staging.to_string_lossy();
        assert!(display.contains("myfile.bin.finalizing."));
        assert!(!staging.exists());
        assert_eq!(staging.parent(), Some(temp.path()));
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn unique_finalizing_path_without_extension() -> TestResult {
        let temp = tempdir()?;
        let dest = temp.path().join("myfile");
        let staging = unique_finalizing_path(&dest)?;

        let display = staging.to_string_lossy();
        assert!(display.contains("myfile.finalizing."));
        assert!(!staging.exists());
        Ok(())
    }

    // ── cleanup_finalizing_paths ────────────────────

    #[timeout(30_000)]
    #[test]
    fn cleanup_finalizing_paths_removes_matching() -> TestResult {
        let temp = tempdir()?;
        let dest = temp.path().join("myfile.bin");

        // Create some finalizing files (matching and non-matching prefixes)
        let f1 = temp.path().join("myfile.bin.finalizing.1234.0.tmp");
        let f2 = temp.path().join("myfile.bin.finalizing.1234.1.tmp");
        let f3 = temp.path().join("otherfile.finalizing.1234.0.tmp");
        let f4 = temp.path().join("unrelated.txt");
        fs::write(&f1, b"")?;
        fs::write(&f2, b"")?;
        fs::write(&f3, b"")?;
        fs::write(&f4, b"")?;

        cleanup_finalizing_paths(&dest)?;

        assert!(!f1.exists(), "matching finalizing file 1 should be removed");
        assert!(!f2.exists(), "matching finalizing file 2 should be removed");
        assert!(f3.exists(), "non-matching finalizing file should remain");
        assert!(f4.exists(), "unrelated file should remain");
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn cleanup_finalizing_paths_no_parent() -> TestResult {
        // A path with no parent (empty) should hit the early-return and be a no-op
        cleanup_finalizing_paths(Path::new(""))?;
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn cleanup_finalizing_paths_read_dir_fails() -> TestResult {
        // Non-existent parent directory → read_dir fails → error returned
        let nonexistent = Path::new(r"\__nonexistent_test_dir__\file.bin");
        let result = cleanup_finalizing_paths(nonexistent);
        assert!(result.is_err(), "expected error for non-existent parent");
        Ok(())
    }

    // ── unique_finalizing_path exhaustion ──────────

    #[timeout(30_000)]
    #[test]
    fn unique_finalizing_path_exhaustion() -> TestResult {
        let temp = tempdir()?;
        let dest = temp.path().join("myfile.bin");
        let pid = std::process::id();

        // Create 1000 files matching all attempt slots (0..1000) to exhaust the loop
        for i in 0..1000u16 {
            let name = format!("myfile.bin.finalizing.{pid}.{i}.tmp");
            let path = temp.path().join(&name);
            fs::write(&path, b"")?;
        }

        let result = unique_finalizing_path(&dest);
        assert!(result.is_err(), "expected exhaustion error");
        match result.unwrap_err() {
            DownloadError::Io(io_err) => {
                assert_eq!(io_err.kind(), std::io::ErrorKind::AlreadyExists);
            }
            other => panic!("expected Io(AlreadyExists), got {other:?}"),
        }
        Ok(())
    }

    // ── fallback_copy_staging_to_destination ───────

    #[timeout(30_000)]
    #[test]
    fn fallback_copy_staging_to_destination_already_exists() -> TestResult {
        let temp = tempdir()?;
        let staging = temp.path().join("staging.tmp");
        let dest = temp.path().join("dest.bin");

        fs::write(&staging, b"staging content")?;
        fs::write(&dest, b"existing content")?;

        let result = fallback_copy_staging_to_destination(&staging, &dest);
        assert!(result.is_err(), "expected error when destination exists");
        match result.unwrap_err() {
            DownloadError::Io(io_err) => {
                assert_eq!(io_err.kind(), std::io::ErrorKind::AlreadyExists);
            }
            other => panic!("expected Io(AlreadyExists), got {other:?}"),
        }
        Ok(())
    }
}
