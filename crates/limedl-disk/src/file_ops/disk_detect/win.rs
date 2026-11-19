use std::collections::HashMap;
use std::ffi::OsStr;
use std::mem;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::sync::OnceLock;

use parking_lot::Mutex;

use super::win_wsl::{resolve_wsl_media, wsl_distro_from_path, wsl_distros};
use crate::types::DiskType;

// IOCTL codes
const IOCTL_STORAGE_QUERY_PROPERTY: u32 = 0x002D1400;
const IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS: u32 = 0x00560000;

const STORAGE_DEVICE_PROPERTY: u32 = 0;
const STORAGE_DEVICE_SEEK_PENALTY_PROPERTY: u32 = 7;
const STORAGE_DEVICE_TRIM_PROPERTY: u32 = 8;

const FILE_READ_ATTRIBUTES: u32 = 0x0080;
const PROPERTY_STANDARD_QUERY: u32 = 0;

// Bus types from STORAGE_BUS_TYPE
const BUS_TYPE_SD: u32 = 12;
const BUS_TYPE_MMC: u32 = 13;
const BUS_TYPE_NVME: u32 = 17;
const BUS_TYPE_SCM: u32 = 18;

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
struct StorageDeviceDescriptor {
    version: u32,
    size: u32,
    device_type: u8,
    device_type_modifier: u8,
    removable_media: u8,
    command_queueing: u8,
    vendor_id_offset: u32,
    product_id_offset: u32,
    product_revision_offset: u32,
    serial_number_offset: u32,
    bus_type: u32,
    raw_properties_length: u32,
    raw_device_properties: [u8; 1],
}

#[repr(C)]
struct DeviceTrimDescriptor {
    version: u32,
    size: u32,
    trim_enabled: u8,
    _reserved: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct DiskExtent {
    disk_number: u32,
    _padding: u32,
    starting_offset: i64,
    extent_length: i64,
}

#[repr(C)]
struct VolumeDiskExtents {
    number_of_disk_extents: u32,
    _padding: u32,
    extents: [DiskExtent; 8],
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

// Network redirector: `Z:` → the UNC target it is mapped to.
#[link(name = "mpr")]
unsafe extern "system" {
    fn WNetGetConnectionW(
        lp_local_name: *const u16,
        lp_remote_name: *mut u16,
        lpn_length: *mut u32,
    ) -> u32;
}

const DRIVE_FIXED: u32 = 3;
const DRIVE_REMOVABLE: u32 = 2;
const DRIVE_REMOTE: u32 = 4;

const INVALID_HANDLE_VALUE: isize = -1;
const FILE_SHARE_READ: u32 = 1;
const FILE_SHARE_WRITE: u32 = 2;
const OPEN_EXISTING: u32 = 3;

const NO_ERROR: u32 = 0;
const ERROR_MORE_DATA: u32 = 234;

/// Convert a string to a null-terminated wide string (UTF-16).
pub(super) fn to_wide_null(s: &str) -> Vec<u16> {
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
fn try_open_volume_inner(wide_path: *const u16, access: u32, volume_path: &str) -> Option<isize> {
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

/// The drive-letter root a Win32 volume API can open, if `path` has one.
///
/// `None` means "not a lettered local volume": UNC shares, device
/// namespaces and ordinary relative paths have no volume to query.
fn drive_letter_for(path: &Path) -> Option<String> {
    match path.components().next()? {
        Component::Prefix(prefix) => drive_letter_from_prefix(prefix.kind()),
        // A relative-but-rooted path (`\downloads`) belongs to the drive the
        // process runs on.
        Component::RootDir => {
            let cwd = std::env::current_dir().ok()?;
            match cwd.components().next()? {
                Component::Prefix(prefix) => drive_letter_from_prefix(prefix.kind()),
                _ => None,
            }
        }
        _ => None,
    }
}

/// `C:` for a disk prefix, `None` for UNC / device / verbatim-UNC prefixes.
fn drive_letter_from_prefix(prefix: Prefix<'_>) -> Option<String> {
    match prefix {
        Prefix::Disk(byte) | Prefix::VerbatimDisk(byte) => {
            Some(format!("{}:", (byte as char).to_ascii_uppercase()))
        }
        _ => None,
    }
}

/// `GetDriveTypeW("Z:\\")`.
fn root_drive_type(drive_letter: &str) -> u32 {
    let root = to_wide_null(&format!("{drive_letter}\\"));
    unsafe { GetDriveTypeW(root.as_ptr()) }
}

/// Resolve a mapped drive letter to its UNC target (`Z:` → `\\server\share`),
/// or `None` for a disconnected mapping.
fn network_path_for_drive(drive_letter: &str) -> Option<String> {
    let local = to_wide_null(drive_letter);
    let mut length: u32 = 260; // MAX_PATH, the documented starting size
    for _ in 0..2 {
        let mut buffer = vec![0u16; length as usize];
        let status =
            unsafe { WNetGetConnectionW(local.as_ptr(), buffer.as_mut_ptr(), &mut length) };
        if status == NO_ERROR {
            let end = buffer
                .iter()
                .position(|unit| *unit == 0)
                .unwrap_or(buffer.len());
            return String::from_utf16(&buffer[..end]).ok();
        }
        // ERROR_MORE_DATA updates `length` with the size that is needed.
        if status != ERROR_MORE_DATA {
            tracing::debug!(
                "disk_detect: WNetGetConnectionW({drive_letter}) failed, status={status}"
            );
            return None;
        }
    }
    None
}

pub fn detect_disk_type(path: &Path) -> DiskType {
    // ── WSL transports ────────────────────────────────────────────────
    // `\\wsl$\Ubuntu\...` is not remote storage: the 9p/virtiofs server
    // inside the utility VM serves a *local* `ext4.vhdx`, so the media
    // question has a real answer here, unlike for a NAS share. Resolve the
    // distro and detect the volume that stores its virtual disk.
    if let Some(distro) = wsl_distro_from_path(path) {
        return resolve_wsl_media(&distro);
    }

    let Some(drive_letter) = drive_letter_for(path) else {
        return match path.components().next() {
            // A share (`\\server\share`), a device namespace or a verbatim
            // UNC path: a remote location, which is a better answer than the
            // "SSD" this used to report silently.
            Some(Component::Prefix(_)) => {
                tracing::debug!("disk_detect: {path:?} is not a local volume, reporting network");
                DiskType::Network
            }
            // An ordinary relative path: no volume to key off, so keep the
            // historical default.
            _ => DiskType::Ssd,
        };
    };

    // Mapped network drives: the redirector exposes no seek-penalty property
    // and `\\.\Z:` cannot be opened as a volume at all, so classify by what
    // the letter points at. A drive mapped to `\\wsl$\...` still resolves to
    // the media of the distro's host volume.
    if root_drive_type(&drive_letter) == DRIVE_REMOTE {
        let media = network_path_for_drive(&drive_letter)
            .and_then(|target| {
                wsl_distro_from_path(Path::new(&target)).map(|distro| resolve_wsl_media(&distro))
            })
            .unwrap_or(DiskType::Network);
        // Deliberately not cached: a letter can be remapped to different
        // storage, and this answer is cheap to recompute.
        tracing::debug!("disk_detect: {drive_letter} is a network drive, media={media:?}");
        return media;
    }

    // Fast path: this volume was already detected this session.
    if let Some(&cached) = disk_type_cache().lock().get(&drive_letter) {
        return cached;
    }

    let handle = match open_volume(&drive_letter) {
        Some(h) => h,
        None => {
            tracing::warn!(
                "disk_detect: could not open volume for {drive_letter}, defaulting to SSD without caching"
            );
            return DiskType::Ssd;
        }
    };

    let result = detect_volume_media(handle, &drive_letter);
    unsafe {
        CloseHandle(handle);
    }

    match result {
        Some(dt) => {
            // Cache the verified result — stable for a mounted volume within this process.
            disk_type_cache().lock().insert(drive_letter.clone(), dt);
            match dt {
                DiskType::Hdd => tracing::debug!("disk_detect: {drive_letter} is HDD"),
                DiskType::Ssd => tracing::debug!("disk_detect: {drive_letter} is SSD"),
                DiskType::Network => {}
            }
            dt
        }
        None => {
            tracing::warn!(
                "disk_detect: could not determine disk type for {drive_letter} after all probes; defaulting to SSD without caching. Consider setting io_baseline.disk_type_overrides in settings."
            );
            DiskType::Ssd
        }
    }
}

/// Open a physical drive handle (e.g., `\\.\PhysicalDrive0`).
fn open_physical_drive(disk_number: u32) -> Option<isize> {
    let path = format!("\\\\.\\PhysicalDrive{disk_number}");
    let wide = to_wide_null(&path);
    // Try zero-access first (avoids admin requirement)
    if let Some(h) = try_open_volume_inner(wide.as_ptr(), 0, &path) {
        return Some(h);
    }
    // Retry with FILE_READ_ATTRIBUTES
    try_open_volume_inner(wide.as_ptr(), FILE_READ_ATTRIBUTES, &path)
}

/// Query the physical disk number for a volume handle via IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS.
fn get_volume_physical_disk_number(volume_handle: isize) -> Option<u32> {
    let mut extents: VolumeDiskExtents = unsafe { mem::zeroed() };
    let mut bytes_returned: u32 = 0;
    let success = unsafe {
        DeviceIoControl(
            volume_handle,
            IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS,
            std::ptr::null(),
            0,
            &mut extents as *mut _ as *mut std::ffi::c_void,
            mem::size_of::<VolumeDiskExtents>() as u32,
            &mut bytes_returned,
            std::ptr::null_mut(),
        )
    };

    if success != 0 && extents.number_of_disk_extents > 0 {
        Some(extents.extents[0].disk_number)
    } else {
        let err = unsafe { GetLastError() };
        tracing::trace!("disk_detect: IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS failed, error={err}");
        None
    }
}

/// Query the Seek Penalty property on a handle (volume or physical drive).
/// IncursSeekPenalty == 0 -> SSD, != 0 -> HDD.
fn query_seek_penalty(handle: isize) -> Option<DiskType> {
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

    if success != 0 && bytes_returned >= 5 {
        if descriptor.incurs_seek_penalty == 0 {
            Some(DiskType::Ssd)
        } else {
            Some(DiskType::Hdd)
        }
    } else {
        let err = unsafe { GetLastError() };
        tracing::trace!("disk_detect: DeviceIoControl(STORAGE_SEEK_PENALTY) failed, error={err}");
        None
    }
}

/// Query the BusType property on a handle (volume or physical drive).
/// NVMe / SCM / SD / MMC are non-rotational flash media -> SSD.
fn query_bus_type(handle: isize) -> Option<DiskType> {
    let query = StoragePropertyQuery {
        property_id: STORAGE_DEVICE_PROPERTY,
        query_type: PROPERTY_STANDARD_QUERY,
        additional_parameters: [0; 1],
    };

    let mut descriptor: StorageDeviceDescriptor = unsafe { mem::zeroed() };
    let mut bytes_returned: u32 = 0;

    let success = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            &query as *const _ as *const std::ffi::c_void,
            mem::size_of::<StoragePropertyQuery>() as u32,
            &mut descriptor as *mut _ as *mut std::ffi::c_void,
            mem::size_of::<StorageDeviceDescriptor>() as u32,
            &mut bytes_returned,
            std::ptr::null_mut(),
        )
    };

    if success != 0 && bytes_returned >= 32 {
        match descriptor.bus_type {
            BUS_TYPE_NVME | BUS_TYPE_SCM | BUS_TYPE_SD | BUS_TYPE_MMC => Some(DiskType::Ssd),
            _ => None,
        }
    } else {
        let err = unsafe { GetLastError() };
        tracing::trace!(
            "disk_detect: DeviceIoControl(STORAGE_DEVICE_PROPERTY) failed, error={err}"
        );
        None
    }
}

/// Query the TRIM property on a handle (volume or physical drive).
/// If TrimEnabled is true, media is flash (SSD).
fn query_trim_enabled(handle: isize) -> Option<DiskType> {
    let query = StoragePropertyQuery {
        property_id: STORAGE_DEVICE_TRIM_PROPERTY,
        query_type: PROPERTY_STANDARD_QUERY,
        additional_parameters: [0; 1],
    };

    let mut descriptor: DeviceTrimDescriptor = unsafe { mem::zeroed() };
    let mut bytes_returned: u32 = 0;

    let success = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            &query as *const _ as *const std::ffi::c_void,
            mem::size_of::<StoragePropertyQuery>() as u32,
            &mut descriptor as *mut _ as *mut std::ffi::c_void,
            mem::size_of::<DeviceTrimDescriptor>() as u32,
            &mut bytes_returned,
            std::ptr::null_mut(),
        )
    };

    if success != 0 && bytes_returned >= 9 && descriptor.trim_enabled != 0 {
        Some(DiskType::Ssd)
    } else {
        let err = unsafe { GetLastError() };
        tracing::trace!(
            "disk_detect: DeviceIoControl(STORAGE_DEVICE_TRIM_PROPERTY) failed, error={err}"
        );
        None
    }
}

/// Probe the media of a physical drive handle using seek penalty, bus type, and trim.
fn query_physical_drive_media(phys_handle: isize) -> Option<DiskType> {
    if let Some(dt) = query_seek_penalty(phys_handle) {
        return Some(dt);
    }
    if let Some(dt) = query_bus_type(phys_handle) {
        return Some(dt);
    }
    if let Some(dt) = query_trim_enabled(phys_handle) {
        return Some(dt);
    }
    None
}

/// Run the fallback detection chain for a volume:
/// 1. Direct Seek Penalty on volume handle
/// 2. Physical Drive Seek Penalty via volume disk extents
/// 3. BusType on volume handle (NVMe -> SSD)
/// 4. TRIM on volume handle (TRIM enabled -> SSD)
fn detect_volume_media(volume_handle: isize, drive_letter: &str) -> Option<DiskType> {
    // 1. Direct Seek Penalty on the volume handle.
    if let Some(dt) = query_seek_penalty(volume_handle) {
        tracing::debug!("disk_detect: {drive_letter} volume seek penalty -> {dt:?}");
        return Some(dt);
    }

    // 2. Physical Drive fallback via volume disk extents.
    if let Some(disk_num) = get_volume_physical_disk_number(volume_handle) {
        tracing::debug!("disk_detect: {drive_letter} mapped to PhysicalDrive{disk_num}");
        if let Some(phys_handle) = open_physical_drive(disk_num) {
            let phys_result = query_physical_drive_media(phys_handle);
            unsafe {
                CloseHandle(phys_handle);
            }
            if let Some(dt) = phys_result {
                tracing::debug!("disk_detect: {drive_letter} (PhysicalDrive{disk_num}) -> {dt:?}");
                return Some(dt);
            }
        } else {
            tracing::debug!("disk_detect: could not open PhysicalDrive{disk_num}");
        }
    }

    // 3. BusType query on volume handle.
    if let Some(dt) = query_bus_type(volume_handle) {
        tracing::debug!("disk_detect: {drive_letter} volume bus type -> {dt:?}");
        return Some(dt);
    }

    // 4. TRIM query on volume handle.
    if let Some(dt) = query_trim_enabled(volume_handle) {
        tracing::debug!("disk_detect: {drive_letter} volume trim -> {dt:?}");
        return Some(dt);
    }

    None
}

/// Enumerate every local, removable and *remote* volume plus each WSL
/// distro, and detect the media type for each.
///
/// Mapped network drives and the WSL transports used to be filtered out of
/// this list, which left the settings panel claiming a machine had no
/// network storage at all — the locations most in need of a manual override
/// were the ones the panel could not name.
pub fn detect_all_disk_types() -> HashMap<String, DiskType> {
    let drives_mask = unsafe { GetLogicalDrives() };
    let mut result = HashMap::new();
    for i in 0..26u32 {
        if drives_mask & (1 << i) == 0 {
            continue;
        }
        let letter = (b'A' + i as u8) as char;
        let drive_root = format!("{letter}:\\");
        let root_wide = to_wide_null(&drive_root);
        let drive_type = unsafe { GetDriveTypeW(root_wide.as_ptr()) };
        if matches!(drive_type, DRIVE_FIXED | DRIVE_REMOVABLE | DRIVE_REMOTE) {
            result.insert(
                format!("{letter}:"),
                detect_disk_type(Path::new(&drive_root)),
            );
        }
    }
    for (distro, media) in wsl_distros() {
        result.insert(format!("\\\\wsl$\\{distro}"), media);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wsl_transports_are_recognised_by_share_name() {
        assert_eq!(
            wsl_distro_from_path(Path::new(r"\\wsl$\Ubuntu\home\me")),
            Some("Ubuntu".to_string())
        );
        // The Windows 11 spelling of the same transport. Both are UNC, so
        // they must not fall into the generic "network share" bucket.
        assert_eq!(
            wsl_distro_from_path(Path::new(r"\\wsl.localhost\Ubuntu\home")),
            Some("Ubuntu".to_string())
        );
        // The server component is case-insensitive.
        assert_eq!(
            wsl_distro_from_path(Path::new(r"\\WSL$\Debian")),
            Some("Debian".to_string())
        );
        // No distro named at all.
        assert_eq!(wsl_distro_from_path(Path::new(r"\\wsl$")), None);

        // Everything else stays a plain location.
        for path in [
            r"\\nas\downloads\file.bin",
            r"C:\downloads",
            r"D:",
            "relative",
        ] {
            assert_eq!(wsl_distro_from_path(Path::new(path)), None, "{path}");
        }
    }

    #[test]
    fn drive_letters_are_read_from_local_paths_only() {
        assert_eq!(
            drive_letter_for(Path::new(r"d:\downloads")),
            Some("D:".to_string())
        );
        assert_eq!(
            drive_letter_for(Path::new(r"\\?\D:\x")),
            Some("D:".to_string())
        );
        // Rooted-but-relative paths resolve against the current drive.
        let current = std::env::current_dir().expect("cwd");
        assert_eq!(
            drive_letter_for(Path::new(r"\downloads")),
            drive_letter_for(&current)
        );
        // Share and relative paths have no volume to open.
        assert_eq!(drive_letter_for(Path::new(r"\\wsl$\Ubuntu\home")), None);
        assert_eq!(drive_letter_for(Path::new(r"\\nas\share")), None);
        assert_eq!(drive_letter_for(Path::new("relative")), None);
    }

    #[test]
    fn unresolvable_wsl_base_path_reports_network() {
        // A missing registry value must not invent a media type — and a
        // `--import --vhd` distro without `ext4.vhdx` falls back to the
        // volume holding the distro itself, which is still a local answer.
        assert_eq!(wsl_media_for_base_path(None), DiskType::Network);
        assert!(matches!(
            wsl_media_for_base_path(Some(r"C:\")),
            DiskType::Ssd | DiskType::Hdd
        ));
    }

    /// End-to-end: a registered distro resolves through its `BasePath` to the
    /// local volume holding its `ext4.vhdx` — that is the whole point of the
    /// WSL branch, since a `\\wsl$` path is *not* remote storage.
    ///
    /// Vacuous on a machine with no distros (every CI runner): it earns its
    /// keep on a developer machine, where the shares may be stopped and
    /// nothing but the registry can answer. The two lookups below go through
    /// different code paths on purpose, so this is a real comparison rather
    /// than a restatement.
    #[test]
    fn registered_wsl_distros_resolve_to_their_host_volume() {
        let distros = wsl_distros();
        for (distro, media) in &distros {
            eprintln!("wsl distro {distro} => {media:?}");
        }
        for (distro, _) in &distros {
            assert_eq!(
                resolve_wsl_media(distro),
                wsl_media_for_base_path(wsl_base_path(distro).as_deref()),
                "{distro}"
            );
        }
    }

    #[test]
    fn unknown_distro_falls_back_to_network_without_a_registry_entry() {
        assert_eq!(
            resolve_wsl_media("limedl-nonexistent-distro"),
            DiskType::Network
        );
    }

    #[test]
    fn network_path_lookup_returns_a_unc_path_or_nothing() {
        // The letters are (almost certainly) unmapped, but the assertion has
        // to hold either way: this is the redirector round-trip, not a
        // predicate we control.
        for letter in ["Q:", "q:", "1:"] {
            if let Some(target) = network_path_for_drive(letter) {
                assert!(target.starts_with(r"\\"), "{letter} resolved to {target}");
            }
        }
    }
}
