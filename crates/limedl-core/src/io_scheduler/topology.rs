//! Physical device topology detection and device identifier resolution.
//!
//! Maps arbitrary file/directory paths to underlying physical storage devices
//! (e.g., `PhysicalDrive0`, `/dev/sda`, `disk0`, UNC share) and identifies the
//! underlying media type (HDD vs SSD).

use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::file_ops::detect_disk_type;
use crate::types::DiskType;

/// Unique identifier for a physical or logical storage device.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id")]
pub enum DeviceId {
    /// Windows physical drive (e.g. `PhysicalDrive0`).
    Physical(String),
    /// Linux parent block device (e.g. `sda`, `nvme0n1`).
    UnixBlock(String),
    /// macOS BSD disk identifier (e.g. `disk0`, `disk1`).
    MacDisk(String),
    /// Fallback logical volume (e.g. `D:`, `/mnt/data`).
    Volume(String),
    /// Network share (e.g. `\\nas\downloads`, `smb://nas/share`).
    Network(String),
    /// Unidentified fallback.
    Unknown(String),
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Physical(id) => write!(f, "physical:{id}"),
            Self::UnixBlock(id) => write!(f, "block:{id}"),
            Self::MacDisk(id) => write!(f, "mac:{id}"),
            Self::Volume(id) => write!(f, "vol:{id}"),
            Self::Network(id) => write!(f, "net:{id}"),
            Self::Unknown(id) => write!(f, "unknown:{id}"),
        }
    }
}

/// Device topology detector with process-lifetime resolution caching.
#[derive(Clone)]
pub struct DeviceTopology {
    cache: Arc<RwLock<HashMap<String, (DeviceId, DiskType)>>>,
}

impl Default for DeviceTopology {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceTopology {
    /// Create a new device topology detector.
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Resolve the underlying physical device ID and disk type for a path.
    pub fn resolve_device(&self, path: &Path) -> (DeviceId, DiskType) {
        let key = path.to_string_lossy().to_string();

        // Fast path: cache hit
        {
            let cache = self.cache.read();
            if let Some(entry) = cache.get(&key) {
                return entry.clone();
            }
        }

        let disk_type = detect_disk_type(path);
        let device_id = imp::detect_device_id(path);

        let mut cache = self.cache.write();
        cache.insert(key, (device_id.clone(), disk_type));
        (device_id, disk_type)
    }
}

// ── Windows Implementation ──────────────────────────────────────────

#[cfg(windows)]
mod imp {
    use std::ffi::OsStr;
    use std::mem;
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Component, Path, Prefix};

    use super::DeviceId;

    const IOCTL_STORAGE_GET_DEVICE_NUMBER: u32 = 0x002D1080;
    const FILE_READ_ATTRIBUTES: u32 = 0x0080;
    const INVALID_HANDLE_VALUE: isize = -1;
    const FILE_SHARE_READ: u32 = 1;
    const FILE_SHARE_WRITE: u32 = 2;
    const OPEN_EXISTING: u32 = 3;

    #[repr(C)]
    struct StorageDeviceNumber {
        device_type: u32,
        device_number: u32,
        partition_number: u32,
    }

    #[repr(C)]
    struct SecurityAttributes {
        n_length: u32,
        lp_security_descriptor: *mut std::ffi::c_void,
        b_inherit_handle: i32,
    }

    unsafe extern "system" {
        fn CreateFileW(
            lp_file_name: *const u16,
            dw_desired_access: u32,
            dw_share_mode: u32,
            lp_security_attributes: *const SecurityAttributes,
            dw_creation_disposition: u32,
            dw_flags_and_attributes: u32,
            h_template_file: *mut std::ffi::c_void,
        ) -> isize;

        fn DeviceIoControl(
            h_device: isize,
            dw_io_control_code: u32,
            lp_in_buffer: *const std::ffi::c_void,
            n_in_buffer_size: u32,
            lp_out_buffer: *mut std::ffi::c_void,
            n_out_buffer_size: u32,
            lp_bytes_returned: *mut u32,
            lp_overlapped: *mut std::ffi::c_void,
        ) -> i32;

        fn CloseHandle(h_object: isize) -> i32;
    }

    fn to_wide_null(s: &str) -> Vec<u16> {
        OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    pub fn detect_device_id(path: &Path) -> DeviceId {
        let first_comp = match path.components().next() {
            Some(Component::Prefix(prefix)) => prefix.kind(),
            _ => return DeviceId::Unknown("relative".into()),
        };

        match first_comp {
            Prefix::Disk(byte) | Prefix::VerbatimDisk(byte) => {
                let letter = (byte as char).to_ascii_uppercase();
                let drive_str = format!("{letter}:");
                let volume_path = format!("\\\\.\\{drive_str}");
                let wide_path = to_wide_null(&volume_path);

                let handle = unsafe {
                    CreateFileW(
                        wide_path.as_ptr(),
                        FILE_READ_ATTRIBUTES,
                        FILE_SHARE_READ | FILE_SHARE_WRITE,
                        std::ptr::null(),
                        OPEN_EXISTING,
                        0,
                        std::ptr::null_mut(),
                    )
                };

                if handle == INVALID_HANDLE_VALUE {
                    return DeviceId::Volume(drive_str);
                }

                let mut dev_number: StorageDeviceNumber = unsafe { mem::zeroed() };
                let mut bytes_returned: u32 = 0;

                let success = unsafe {
                    DeviceIoControl(
                        handle,
                        IOCTL_STORAGE_GET_DEVICE_NUMBER,
                        std::ptr::null(),
                        0,
                        &mut dev_number as *mut _ as *mut std::ffi::c_void,
                        mem::size_of::<StorageDeviceNumber>() as u32,
                        &mut bytes_returned,
                        std::ptr::null_mut(),
                    )
                };

                unsafe { CloseHandle(handle); }

                if success != 0 && bytes_returned >= mem::size_of::<StorageDeviceNumber>() as u32 {
                    DeviceId::Physical(format!("PhysicalDrive{}", dev_number.device_number))
                } else {
                    DeviceId::Volume(drive_str)
                }
            }
            Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => {
                let s = server.to_string_lossy();
                let sh = share.to_string_lossy();
                DeviceId::Network(format!("\\\\{s}\\{sh}"))
            }
            _ => DeviceId::Unknown("unsupported_prefix".into()),
        }
    }
}

// ── Linux Implementation ─────────────────────────────────────────────

#[cfg(target_os = "linux")]
mod imp {
    use std::fs;
    use std::path::Path;

    use super::DeviceId;

    pub fn detect_device_id(path: &Path) -> DeviceId {
        let Ok(meta) = fs::metadata(path) else {
            return DeviceId::Unknown("stat_failed".into());
        };

        use std::os::linux::fs::MetadataExt;
        let dev = meta.st_dev();
        let major = libc::major(dev);
        let minor = libc::minor(dev);

        let dev_symlink = format!("/sys/dev/block/{major}:{minor}");
        let Ok(link) = fs::read_link(&dev_symlink) else {
            return DeviceId::Volume(format!("dev_{major}_{minor}"));
        };

        if let Some(device_name) = link.file_name().and_then(|n| n.to_str()) {
            // Strip partition numbers to find the base physical disk (e.g. sda1 -> sda, nvme0n1p1 -> nvme0n1)
            let base_name = strip_partition(device_name);
            DeviceId::UnixBlock(base_name.to_string())
        } else {
            DeviceId::Volume(format!("dev_{major}_{minor}"))
        }
    }

    fn strip_partition(name: &str) -> &str {
        if name.starts_with("nvme") || name.starts_with("mmcblk") {
            if let Some(pos) = name.rfind('p')
                && name[pos + 1..].chars().all(|c| c.is_ascii_digit())
            {
                return &name[..pos];
            }
        } else if let Some(pos) = name.find(|c: char| c.is_ascii_digit()) {
            return &name[..pos];
        }
        name
    }
}

// ── macOS Implementation ─────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod imp {
    use std::ffi::CStr;
    use std::path::Path;

    use super::DeviceId;

    pub fn detect_device_id(path: &Path) -> DeviceId {
        let Ok(abs_path) = std::fs::canonicalize(path) else {
            return DeviceId::Unknown("canonicalize_failed".into());
        };

        let Ok(path_c) = std::ffi::CString::new(abs_path.to_string_lossy().as_bytes()) else {
            return DeviceId::Unknown("invalid_cstring".into());
        };

        let mut fsbuf: libc::statfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statfs(path_c.as_ptr(), &mut fsbuf) } != 0 {
            return DeviceId::Unknown("statfs_failed".into());
        }

        let mntfrom = unsafe { CStr::from_ptr(fsbuf.f_mntfromname.as_ptr()) };
        let dev_path_str = mntfrom.to_string_lossy();

        if let Some(file_name) = std::path::Path::new(dev_path_str.as_ref()).file_name() {
            let bsd_full = file_name.to_string_lossy().to_string();
            let base_disk = if let Some(s_idx) = bsd_full.find('s') {
                &bsd_full[..s_idx]
            } else {
                &bsd_full
            };
            DeviceId::MacDisk(base_disk.to_string())
        } else {
            DeviceId::Volume("macos_root".into())
        }
    }
}

// ── Fallback Implementation ──────────────────────────────────────────

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
mod imp {
    use std::path::Path;
    use super::DeviceId;

    pub fn detect_device_id(path: &Path) -> DeviceId {
        DeviceId::Volume(path.to_string_lossy().to_string())
    }
}
