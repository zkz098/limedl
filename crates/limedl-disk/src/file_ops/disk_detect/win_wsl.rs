use std::collections::HashMap;
use std::path::{Component, Path, Prefix};
use std::sync::OnceLock;

use parking_lot::Mutex;

use super::imp::{detect_disk_type, to_wide_null};
use crate::types::DiskType;

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

// `HKEY_CURRENT_USER` is the sign-extended pseudo-handle 0x80000001.
const HKEY_CURRENT_USER: isize = 0x8000_0001u32 as i32 as isize;
/// `KEY_QUERY_VALUE | KEY_ENUMERATE_SUB_KEYS` plus the standard read rights.
const KEY_READ: u32 = 0x0002_0019;
const REG_SZ: u32 = 1;
/// The registry APIs return `LONG`.
const ERROR_SUCCESS: i32 = 0;
/// Key holding one GUID-named subkey per registered WSL distribution.
const LXSS_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Lxss";

/// `\\wsl$\Ubuntu\home\me` / `\\wsl.localhost\Ubuntu\...` → `Some("Ubuntu")`.
pub(super) fn wsl_distro_from_path(path: &Path) -> Option<String> {
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

pub(super) fn resolve_wsl_media(distro: &str) -> DiskType {
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
pub(super) fn wsl_distros() -> Vec<(String, DiskType)> {
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
        wsl_media_cache()
            .lock()
            .insert(name.to_ascii_lowercase(), media);
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
    let status = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, wide.as_ptr(), 0, KEY_READ, &mut key) };
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
    let end = buffer
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(buffer.len());
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
