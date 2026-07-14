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
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    /// Open the volume handle for a drive letter (e.g., "C:" → `\\.\C:`).
    /// Uses dwDesiredAccess=0 to avoid requiring administrator privileges —
    /// IOCTL_STORAGE_QUERY_PROPERTY only needs a query handle.
    fn open_volume(drive_letter: &str) -> Option<isize> {
        let volume_path = format!("\\\\.\\{drive_letter}");
        let wide_path = to_wide_null(&volume_path);

        let handle = unsafe {
            CreateFileW(
                wide_path.as_ptr(),
                0, // no access needed; avoids admin requirement
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };

        if handle == INVALID_HANDLE_VALUE {
            let err = unsafe { GetLastError() };
            tracing::warn!(
                "disk_detect: CreateFileW({volume_path}) failed, error={err}, \
                 falling back to SSD default (try running as admin)"
            );
            None
        } else {
            Some(handle)
        }
    }

    pub(crate) fn detect_disk_type(path: &Path) -> DiskType {
        // Get the drive letter or mount point root
        let drive_letter = match path.components().next() {
            Some(std::path::Component::Prefix(prefix)) => {
                match prefix.kind() {
                    std::path::Prefix::Disk(byte) | std::path::Prefix::VerbatimDisk(byte) => {
                        format!("{}:", byte as char)
                    }
                    _ => return DiskType::Ssd, // UNC paths, device namespace — fallback
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
            None => return DiskType::Ssd,
        };

        let result = query_seek_penalty(handle);

        // Always close the handle
        unsafe {
            CloseHandle(handle);
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

#[cfg(not(windows))]
mod imp {
    use std::path::Path;

    use super::DiskType;

    pub(crate) fn detect_disk_type(_path: &Path) -> DiskType {
        DiskType::Ssd
    }
}

pub(crate) use imp::detect_disk_type;
