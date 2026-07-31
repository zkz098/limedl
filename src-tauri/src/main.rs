// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() {
    apply_graphics_workarounds();
    limedl_lib::run()
}

/// Workaround for "Gdk-Message: Error 71 (Protocol error) dispatching to Wayland
/// display" startup crashes on Linux with NVIDIA proprietary drivers + Wayland.
///
/// WebKitGTK's DMABUF renderer commits to `wp_linux_drm_syncobj_surface_v1`
/// without setting an acquire point, which violates the explicit-sync protocol
/// and makes the compositor kill the connection (tauri#10702, WebKit bug 280210).
/// Upstream marked this WONTFIX; the documented, zero-cost workaround is
/// disabling explicit sync. See https://v2.tauri.app/develop/debug/linux-graphics/
///
/// Must run before GTK/WebKit initialization. User-set values always win.
fn apply_graphics_workarounds() {
    if cfg!(target_os = "linux")
        && std::env::var_os("WAYLAND_DISPLAY").is_some()
        && std::env::var_os("__NV_DISABLE_EXPLICIT_SYNC").is_none()
        && std::path::Path::new("/proc/driver/nvidia/version").exists()
    {
        // SAFETY: single-threaded, runs before any other thread or GTK init.
        unsafe { std::env::set_var("__NV_DISABLE_EXPLICIT_SYNC", "1") };
    }
}
