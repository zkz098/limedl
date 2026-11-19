// ── Disk Detection ────────────────────────────────────

#[cfg(windows)]
#[path = "win.rs"]
mod imp;
#[cfg(windows)]
mod win_wsl;

#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod imp;

#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod imp;

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
#[path = "other.rs"]
mod imp;

pub use imp::detect_all_disk_types;
pub use imp::detect_disk_type;
