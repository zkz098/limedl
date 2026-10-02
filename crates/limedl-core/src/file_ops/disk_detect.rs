// ── Disk Detection ────────────────────────────────────

#[cfg(windows)]
mod imp {
    use std::collections::HashMap;
    use std::ffi::OsStr;
    use std::mem;
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Component, Path, Prefix};
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

    // WSL distro lookup. `HKCU\Software\Microsoft\Windows\CurrentVersion\Lxss`
    // is the only place that maps a distro name to its virtual disk; the share
    // name in `\\wsl$\<distro>` is not a filesystem path the volume APIs can
    // open, so there is nothing to query without the registry.
    unsafe extern "system" {
        fn RegOpenKeyExW(
            h_key: isize,
            lp_sub_key: *const u16,
            ul_options: u32,
            sam_desired: u32,
            phk_result: *mut isize,
        ) -> i32;

        fn RegEnumKeyExW(
            h_key: isize,
            dw_index: u32,
            lp_name: *mut u16,
            lpcch_name: *mut u32,
            lp_reserved: *mut u32,
            lp_class: *mut u16,
            lpcch_class: *mut u32,
            lpft_last_write_time: *mut std::ffi::c_void,
        ) -> i32;

        fn RegQueryValueExW(
            h_key: isize,
            lp_value_name: *const u16,
            lp_reserved: *mut u32,
            lp_type: *mut u32,
            lp_data: *mut u8,
            lpcb_data: *mut u32,
        ) -> i32;

        fn RegCloseKey(h_key: isize) -> i32;
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

    // `HKEY_CURRENT_USER` is the sign-extended pseudo-handle 0x80000001.
    const HKEY_CURRENT_USER: isize = 0x8000_0001u32 as i32 as isize;
    /// `KEY_QUERY_VALUE | KEY_ENUMERATE_SUB_KEYS` plus the standard read rights.
    const KEY_READ: u32 = 0x0002_0019;
    const REG_SZ: u32 = 1;
    /// The registry APIs return `LONG`.
    const ERROR_SUCCESS: i32 = 0;
    /// The shell/redirector APIs return `DWORD`.
    const NO_ERROR: u32 = 0;
    const ERROR_MORE_DATA: u32 = 234;
    /// Key holding one GUID-named subkey per registered WSL distribution.
    const LXSS_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Lxss";

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
                let end = buffer.iter().position(|unit| *unit == 0).unwrap_or(buffer.len());
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

    /// `\\wsl$\Ubuntu\home\me` / `\\wsl.localhost\Ubuntu\...` → `Some("Ubuntu")`.
    fn wsl_distro_from_path(path: &Path) -> Option<String> {
        let Some(Component::Prefix(prefix)) = path.components().next() else {
            return None;
        };
        let (server, share) = match prefix.kind() {
            Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => (server, share),
            _ => return None,
        };
        let server = server.to_string_lossy().to_ascii_lowercase();
        if server != "wsl$" && server != "wsl.localhost" {
            return None;
        }
        let share = share.to_string_lossy().to_string();
        // `\\wsl$\` on its own addresses no distro.
        (!share.is_empty()).then_some(share)
    }

    /// Distro name → host media, resolved once per process: the Lxss → VHDX
    /// mapping does not change while limedl runs, and resolving it walks the
    /// registry (several syscalls) on every download start otherwise.
    fn wsl_media_cache() -> &'static Mutex<HashMap<String, DiskType>> {
        static CACHE: OnceLock<Mutex<HashMap<String, DiskType>>> = OnceLock::new();
        CACHE.get_or_init(|| Mutex::new(HashMap::new()))
    }

    fn resolve_wsl_media(distro: &str) -> DiskType {
        let cache_key = distro.to_ascii_lowercase();
        if let Some(&cached) = wsl_media_cache().lock().get(&cache_key) {
            return cached;
        }
        let Some(base_path) = wsl_base_path(distro) else {
            // Unknown distro (never registered, or the registry is unreadable).
            // The path is still a remote transport, so report it as one — and do
            // not cache the miss, because the distro may exist by the next
            // download.
            tracing::warn!(
                "disk_detect: no Lxss registry entry for WSL distro {distro}, reporting network"
            );
            return DiskType::Network;
        };
        let media = wsl_media_for_base_path(Some(&base_path));
        wsl_media_cache().lock().insert(cache_key, media);
        tracing::debug!("disk_detect: \\wsl$\\{distro} media={media:?} (base_path={base_path})");
        media
    }

    /// The `BasePath` of a registered distro.
    fn wsl_base_path(distro: &str) -> Option<String> {
        let lxss = reg_open(LXSS_KEY)?;
        let base_path = reg_subkeys(lxss).into_iter().find_map(|subkey| {
            let entry = reg_open(&format!("{LXSS_KEY}\\{subkey}"))?;
            let name = reg_read_string(entry, "DistributionName");
            let base = reg_read_string(entry, "BasePath");
            unsafe { RegCloseKey(entry) };
            if name.is_some_and(|name| name.eq_ignore_ascii_case(distro)) {
                return base;
            }
            None
        });
        unsafe { RegCloseKey(lxss) };
        base_path
    }

    /// Every registered distro with the media of the volume backing it.
    fn wsl_distros() -> Vec<(String, DiskType)> {
        let Some(lxss) = reg_open(LXSS_KEY) else {
            return Vec::new();
        };
        let mut distros = Vec::new();
        for subkey in reg_subkeys(lxss) {
            let Some(entry) = reg_open(&format!("{LXSS_KEY}\\{subkey}")) else {
                continue;
            };
            let name = reg_read_string(entry, "DistributionName");
            let base_path = reg_read_string(entry, "BasePath");
            unsafe { RegCloseKey(entry) };
            let Some(name) = name else { continue };
            let media = wsl_media_for_base_path(base_path.as_deref());
            // Feed the per-distro cache so a subsequent download to the same
            // distro does not walk the registry again.
            wsl_media_cache().lock().insert(name.to_ascii_lowercase(), media);
            distros.push((name, media));
        }
        unsafe { RegCloseKey(lxss) };
        distros
    }

    /// Media of the local volume that stores a distro's files.
    ///
    /// WSL2 keeps the root filesystem in `<BasePath>\ext4.vhdx` — a plain file
    /// on a Windows volume, so the ordinary seek-penalty query answers
    /// correctly. A `wsl --import --vhd` layout has no `ext4.vhdx`, and a WSL1
    /// distro has no virtual disk at all; both keep their files directly on the
    /// host volume, which is `BasePath` itself. A missing `BasePath` means the
    /// distro could not be resolved.
    fn wsl_media_for_base_path(base_path: Option<&str>) -> DiskType {
        let Some(base_path) = base_path else {
            return DiskType::Network;
        };
        let base = Path::new(base_path);
        // `BasePath` is stored with a trailing separator; `join` copes with both
        // spellings.
        let vhdx = base.join("ext4.vhdx");
        if vhdx.is_file() {
            detect_disk_type(&vhdx)
        } else {
            detect_disk_type(base)
        }
    }

    /// Open a `HKCU` subkey for reading.
    fn reg_open(subkey: &str) -> Option<isize> {
        let wide = to_wide_null(subkey);
        let mut key: isize = 0;
        let status =
            unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, wide.as_ptr(), 0, KEY_READ, &mut key) };
        if status == ERROR_SUCCESS {
            Some(key)
        } else {
            tracing::debug!("disk_detect: RegOpenKeyExW({subkey}) status={status}");
            None
        }
    }

    /// Read a `REG_SZ` value; `None` for missing values and for other types.
    fn reg_read_string(key: isize, name: &str) -> Option<String> {
        let wide = to_wide_null(name);
        // Probe for the size first: `BasePath` is longer than any buffer worth
        // guessing with, and the probe keeps the read exact.
        let mut size: u32 = 0;
        let mut value_type: u32 = 0;
        let status = unsafe {
            RegQueryValueExW(
                key,
                wide.as_ptr(),
                std::ptr::null_mut(),
                &mut value_type,
                std::ptr::null_mut(),
                &mut size,
            )
        };
        // A `REG_SZ` is at least the two bytes of its terminator.
        if status != ERROR_SUCCESS || value_type != REG_SZ || size < 2 {
            return None;
        }
        let mut buffer = vec![0u16; (size as usize).div_ceil(2) + 1];
        let status = unsafe {
            RegQueryValueExW(
                key,
                wide.as_ptr(),
                std::ptr::null_mut(),
                &mut value_type,
                buffer.as_mut_ptr() as *mut u8,
                &mut size,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        let end = buffer.iter().position(|unit| *unit == 0).unwrap_or(buffer.len());
        String::from_utf16(&buffer[..end]).ok()
    }

    /// Subkey names of `key` (the Lxss children are per-distro GUIDs).
    fn reg_subkeys(key: isize) -> Vec<String> {
        let mut names = Vec::new();
        for index in 0.. {
            let mut buffer = [0u16; 256];
            let mut length = buffer.len() as u32;
            let status = unsafe {
                RegEnumKeyExW(
                    key,
                    index,
                    buffer.as_mut_ptr(),
                    &mut length,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            };
            // ERROR_NO_MORE_ITEMS (and any other failure) ends the walk.
            if status != ERROR_SUCCESS {
                break;
            }
            if let Ok(name) = String::from_utf16(&buffer[..length as usize]) {
                names.push(name);
            }
        }
        names
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
                    tracing::debug!(
                        "disk_detect: {path:?} is not a local volume, reporting network"
                    );
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
                    wsl_distro_from_path(Path::new(&target))
                        .map(|distro| resolve_wsl_media(&distro))
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
            // Unreachable: `query_seek_penalty` only answers Hdd/Ssd.
            DiskType::Network => {}
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
                result.insert(format!("{letter}:"), detect_disk_type(Path::new(&drive_root)));
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
            for path in [r"\\nas\downloads\file.bin", r"C:\downloads", r"D:", "relative"] {
                assert_eq!(wsl_distro_from_path(Path::new(path)), None, "{path}");
            }
        }

        #[test]
        fn drive_letters_are_read_from_local_paths_only() {
            assert_eq!(drive_letter_for(Path::new(r"d:\downloads")), Some("D:".to_string()));
            assert_eq!(drive_letter_for(Path::new(r"\\?\D:\x")), Some("D:".to_string()));
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
            assert_eq!(resolve_wsl_media("limedl-nonexistent-distro"), DiskType::Network);
        }

        #[test]
        fn network_path_lookup_returns_a_unc_path_or_nothing() {            // The letters are (almost certainly) unmapped, but the assertion has
            // to hold either way: this is the redirector round-trip, not a
            // predicate we control.
            for letter in ["Q:", "q:", "1:"] {
                if let Some(target) = network_path_for_drive(letter) {
                    assert!(target.starts_with(r"\\"), "{letter} resolved to {target}");
                }
            }
        }
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use std::fs;
    use std::path::Path;

    use crate::file_ops::{is_network_filesystem, parse_mountinfo, path_is_within};
    use crate::types::DiskType;

    pub fn detect_disk_type(path: &Path) -> DiskType {
        // Network and host-brokered mounts have no local block device to
        // inspect, so every check below would answer "SSD" for an NFS share —
        // and for WSL's `/mnt/c`, which is a 9p transport over a VHDX whose
        // media the guest cannot see. Classify those first.
        if let Some(media) = network_mount_media(path) {
            return media;
        }

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

    /// Media for `path` when it lives on a network or host-brokered mount.
    fn network_mount_media(path: &Path) -> Option<DiskType> {
        let absolute = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let mountinfo = fs::read_to_string("/proc/self/mountinfo").ok()?;

        let mut best: Option<usize> = None;
        let mut is_network = false;
        for (mount_point, fstype) in parse_mountinfo(&mountinfo) {
            if !path_is_within(&absolute, Path::new(&mount_point)) {
                continue;
            }
            // The longest matching mount point wins: `/mnt/nas/sub` is a
            // different filesystem from `/mnt`.
            if best.is_none_or(|length| mount_point.len() > length) {
                best = Some(mount_point.len());
                is_network = is_network_filesystem(&fstype);
            }
        }

        (best.is_some() && is_network).then_some(DiskType::Network)
    }

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
        // Block devices cannot describe a network mount, but they are exactly
        // the locations a user needs to be able to name before pinning one with
        // a media override.
        if let Ok(mountinfo) = fs::read_to_string("/proc/self/mountinfo") {
            for (mount_point, fstype) in parse_mountinfo(&mountinfo) {
                if is_network_filesystem(&fstype) {
                    result.insert(mount_point, DiskType::Network);
                }
            }
        }
        result
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// The mountinfo parser and the component-boundary match are tested in
        /// `file_ops::media` — they are platform-independent, and that is where
        /// the Windows gate can actually compile them. This pins the Linux
        /// wiring only: a local path must not be classified as remote, and a
        /// local filesystem must still be probed from its block device.
        #[test]
        fn local_paths_are_not_network_mounts() {
            assert_eq!(network_mount_media(Path::new("/tmp")), None);
            assert_ne!(detect_disk_type(Path::new("/")), DiskType::Network);
        }
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

        // A network mount has no IOKit media of its own: the "Rotational" probe
        // below would either find nothing or describe unrelated hardware, so
        // classify the mount by its filesystem type instead.
        let fstype = unsafe { CStr::from_ptr(fsbuf.f_fstypename.as_ptr()) };
        if crate::file_ops::is_network_filesystem(&fstype.to_string_lossy()) {
            return DiskType::Network;
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
