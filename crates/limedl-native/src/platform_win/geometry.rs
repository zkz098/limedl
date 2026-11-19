use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(windows)]
use windows::Win32::Foundation::{HWND, POINT, RECT};
#[cfg(windows)]
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONULL, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromRect,
    MonitorFromWindow,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowPlacement, GetWindowRect, IsIconic, IsZoomed, SW_HIDE, SW_SHOWMAXIMIZED,
    SW_SHOWNORMAL, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPlacement, SetWindowPos,
    WINDOWPLACEMENT, WINDOWPLACEMENT_FLAGS, WPF_RESTORETOMAXIMIZED,
};

/// Persisted window geometry (position, size, maximized status), in physical
/// pixels — the same unit Slint reports from `Window::position`/`Window::size`
/// and Win32 from `GetWindowPlacement`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_maximized: bool,
}

/// Upper bound for a plausible saved window coordinate.
///
/// Non-Windows platforms cannot enumerate monitors through Slint, so an
/// off-screen-rect check like the Win32 `MonitorFromRect` one is unavailable.
/// Instead a saved position is rejected as stale when it falls outside this
/// bound: no real multi-monitor layout (up to a 6K/8K display chain) reaches
/// ±20k physical pixels, while a window that lived on a since-disconnected
/// monitor keeps its old virtual-desktop coordinates and does.
#[cfg(not(windows))]
const MAX_PLAUSIBLE_COORD: i32 = 20_000;

/// Smallest restorable window, in physical pixels (mirrors the filter the
/// Win32 path applies to `GetWindowPlacement`).
pub(super) const MIN_WINDOW_WIDTH: u32 = 600;
pub(super) const MIN_WINDOW_HEIGHT: u32 = 400;

pub const WINDOW_STATE_FILE: &str = "window_state.json";
static BASE_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);
static LAST_SAVED_GEOMETRY: Mutex<Option<WindowGeometry>> = Mutex::new(None);

/// Configure base directory for persisting window geometry.
pub fn set_base_dir(dir: PathBuf) {
    *BASE_DIR.lock() = Some(dir);
}

/// Get configured base directory for persisting window geometry.
pub fn get_base_dir() -> Option<PathBuf> {
    BASE_DIR.lock().clone()
}

/// Load saved window geometry from disk.
pub fn load_window_geometry(base_dir: &Path) -> Option<WindowGeometry> {
    let path = base_dir.join(WINDOW_STATE_FILE);
    let data = std::fs::read_to_string(path).ok()?;
    let geom: WindowGeometry = serde_json::from_str(&data).ok()?;
    if geom.width >= MIN_WINDOW_WIDTH && geom.height >= MIN_WINDOW_HEIGHT {
        Some(geom)
    } else {
        None
    }
}

/// Save window geometry to disk, skipping if unchanged.
pub fn save_window_geometry(base_dir: &Path, geom: &WindowGeometry) {
    let mut last = LAST_SAVED_GEOMETRY.lock();
    if last.as_ref() == Some(geom) {
        return;
    }
    let path = base_dir.join(WINDOW_STATE_FILE);
    if let Ok(json) = serde_json::to_string_pretty(geom) {
        let _ = std::fs::write(path, json);
        *last = Some(geom.clone());
    }
}

#[cfg(windows)]
pub(super) fn save_geometry_if_configured(hwnd: HWND) {
    if let Some(base_dir) = BASE_DIR.lock().clone()
        && let Some(geom) = capture_window_geometry(hwnd)
    {
        save_window_geometry(&base_dir, &geom);
    }
}

/// Capture normal (unmaximized) geometry and current maximized state.
#[cfg(windows)]
pub fn capture_window_geometry(hwnd: HWND) -> Option<WindowGeometry> {
    unsafe {
        let mut wp = WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };
        if GetWindowPlacement(hwnd, &mut wp).is_ok() {
            let is_maximized = if IsIconic(hwnd).as_bool() {
                wp.flags.contains(WPF_RESTORETOMAXIMIZED)
            } else {
                IsZoomed(hwnd).as_bool()
            };
            let rect = wp.rcNormalPosition;
            let width = (rect.right - rect.left).max(0) as u32;
            let height = (rect.bottom - rect.top).max(0) as u32;
            if width >= MIN_WINDOW_WIDTH && height >= MIN_WINDOW_HEIGHT {
                return Some(WindowGeometry {
                    x: rect.left,
                    y: rect.top,
                    width,
                    height,
                    is_maximized,
                });
            }
        }
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_ok() {
            let width = (rect.right - rect.left).max(0) as u32;
            let height = (rect.bottom - rect.top).max(0) as u32;
            let is_maximized = IsZoomed(hwnd).as_bool();
            if width >= MIN_WINDOW_WIDTH && height >= MIN_WINDOW_HEIGHT {
                return Some(WindowGeometry {
                    x: rect.left,
                    y: rect.top,
                    width,
                    height,
                    is_maximized,
                });
            }
        }
        None
    }
}

/// Centers the window horizontally and vertically within the monitor's work area (excluding taskbar).
#[cfg(windows)]
pub fn center_window_on_monitor(hwnd: HWND) {
    unsafe {
        let hmonitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY);
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(hmonitor, &mut mi).as_bool() {
            let work = mi.rcWork;
            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect).is_ok() {
                let mut win_w = (rect.right - rect.left).max(0) as u32;
                let mut win_h = (rect.bottom - rect.top).max(0) as u32;
                let work_w = (work.right - work.left).max(0) as u32;
                let work_h = (work.bottom - work.top).max(0) as u32;
                let resize = win_w > work_w || win_h > work_h;
                if win_w > work_w {
                    win_w = work_w;
                }
                if win_h > work_h {
                    win_h = work_h;
                }
                let x = work.left + ((work_w - win_w) as i32 / 2).max(0);
                let y = work.top + ((work_h - win_h) as i32 / 2).max(0);
                let flags = if resize {
                    SWP_NOZORDER | SWP_NOACTIVATE
                } else {
                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE
                };
                let _ = SetWindowPos(hwnd, None, x, y, win_w as i32, win_h as i32, flags);
                tracing::info!(
                    "窗口已智能居中于显示器工作区: x={x}, y={y}, w={win_w}, h={win_h} (工作区 {work_w}x{work_h})"
                );
            }
        }
    }
}

/// Clamps a restored window geometry into a monitor's work area, preventing oversized
/// windows (e.g. saved on a 4K display and restored on a 1080p or 768p laptop display)
/// or windows whose title bars/edges hang outside the usable monitor bounds.
#[cfg(any(windows, test))]
pub fn clamp_geometry_to_work_area(
    geom: &WindowGeometry,
    work_left: i32,
    work_top: i32,
    work_right: i32,
    work_bottom: i32,
) -> WindowGeometry {
    let work_w = (work_right - work_left).max(0) as u32;
    let work_h = (work_bottom - work_top).max(0) as u32;
    let width = geom.width.min(work_w).max(MIN_WINDOW_WIDTH.min(work_w));
    let height = geom.height.min(work_h).max(MIN_WINDOW_HEIGHT.min(work_h));
    let max_x = (work_right - width as i32).max(work_left);
    let max_y = (work_bottom - height as i32).max(work_top);
    let x = geom.x.clamp(work_left, max_x);
    let y = geom.y.clamp(work_top, max_y);
    WindowGeometry {
        x,
        y,
        width,
        height,
        is_maximized: geom.is_maximized,
    }
}

/// True when a saved geometry is an on-screen rectangle this platform can use.
#[cfg(windows)]
pub fn is_geometry_visible_on_any_monitor(geom: &WindowGeometry) -> bool {
    unsafe {
        let check_w = geom.width.min(100) as i32;
        let check_h = geom.height.min(40) as i32;
        let rect = RECT {
            left: geom.x,
            top: geom.y,
            right: geom.x + check_w,
            bottom: geom.y + check_h,
        };
        let hmonitor = MonitorFromRect(&rect, MONITOR_DEFAULTTONULL);
        !hmonitor.is_invalid()
    }
}

/// True when a saved geometry is plausible for the current arrangement.
///
/// Deliberately weaker than the Win32 variant: see [`MAX_PLAUSIBLE_COORD`].
#[cfg(not(windows))]
pub fn is_geometry_visible_on_any_monitor(geom: &WindowGeometry) -> bool {
    geom.x.abs() <= MAX_PLAUSIBLE_COORD
        && geom.y.abs() <= MAX_PLAUSIBLE_COORD
        && geom.width >= MIN_WINDOW_WIDTH
        && geom.height >= MIN_WINDOW_HEIGHT
}

/// Apply the saved window placement (position / size / maximized), or leave
/// placement to the OS when there is nothing usable to restore.
///
/// Returns `true` once the placement is applied (or nothing is saved) and the
/// caller should stop retrying. `main.rs` retries because the OS window does not
/// exist until Slint starts the event loop: on Windows the HWND is missing, and
/// on the other platforms the winit backends may drop a `set_position` that
/// arrives before the window is shown.
///
/// `current_maximized` is the window's live maximized state, which decides
/// whether a missing maximize still needs to be (re-)requested.
///
/// On Windows this also centers the window on the monitor work area when the
/// saved rectangle is off-screen, because Win32 lets us query it. The other
/// platforms only skip the restore and let the window manager place the window
/// (Slint exposes no monitor geometry to center against); that is the better
/// default there anyway — winit/Cocoa/GTK apply their own cascade placement.
pub fn apply_restored_window_placement(
    window: &slint::Window,
    base_dir: &Path,
    current_maximized: bool,
) -> bool {
    let Some(geom) = load_window_geometry(base_dir) else {
        return true;
    };

    if !is_geometry_visible_on_any_monitor(&geom) {
        tracing::warn!("已保存的窗口位置不在当前显示器布局中，交由窗口管理器放置");
        #[cfg(windows)]
        if let Some(hwnd) = win32_window_handle(window) {
            center_window_on_monitor(hwnd);
        }
        return true;
    }

    #[cfg(windows)]
    {
        let Some(hwnd) = win32_window_handle(window) else {
            // Slint has not created the OS window yet — the caller retries.
            tracing::debug!("原生窗口句柄尚未就绪，稍后恢复窗口位置");
            return false;
        };
        unsafe {
            let check_w = geom.width.min(100) as i32;
            let check_h = geom.height.min(40) as i32;
            let rect = RECT {
                left: geom.x,
                top: geom.y,
                right: geom.x + check_w,
                bottom: geom.y + check_h,
            };
            let hmonitor = MonitorFromRect(&rect, MONITOR_DEFAULTTOPRIMARY);
            let mut mi = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            let clamped = if GetMonitorInfoW(hmonitor, &mut mi).as_bool() {
                clamp_geometry_to_work_area(
                    &geom,
                    mi.rcWork.left,
                    mi.rcWork.top,
                    mi.rcWork.right,
                    mi.rcWork.bottom,
                )
            } else {
                geom.clone()
            };

            let show_cmd = if !window.is_visible() {
                SW_HIDE.0 as u32
            } else if clamped.is_maximized {
                SW_SHOWMAXIMIZED.0 as u32
            } else {
                SW_SHOWNORMAL.0 as u32
            };
            let flags = if clamped.is_maximized {
                WPF_RESTORETOMAXIMIZED
            } else {
                WINDOWPLACEMENT_FLAGS::default()
            };
            let wp = WINDOWPLACEMENT {
                length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
                flags,
                showCmd: show_cmd,
                ptMinPosition: POINT { x: -1, y: -1 },
                ptMaxPosition: POINT { x: -1, y: -1 },
                rcNormalPosition: RECT {
                    left: clamped.x,
                    top: clamped.y,
                    right: clamped.x + clamped.width as i32,
                    bottom: clamped.y + clamped.height as i32,
                },
            };
            let _ = SetWindowPlacement(hwnd, &wp);
        }
    }

    #[cfg(not(windows))]
    {
        window.set_size(slint::PhysicalSize::new(geom.width, geom.height));
        window.set_position(slint::PhysicalPosition::new(geom.x, geom.y));
        if geom.is_maximized && window.is_visible() {
            window.set_maximized(true);
        }
    }

    // If the window is currently hidden, placement was applied without displaying it.
    // Maximization will be applied when the window is actually shown (e.g. from the tray).
    if !window.is_visible() {
        return true;
    }

    // On Windows `is_maximized` follows the OS window, so a maximize request that
    // was dropped before the window existed shows up here and keeps the retry
    // alive. The `is_visible` half stops the retry once the window is really up,
    // so it cannot fight a user who restores a maximized window by hand.
    let still_missing_placement = geom.is_maximized && !current_maximized && window.is_minimized();

    tracing::info!(
        "窗口位置已恢复: ({}, {}) {}x{} (最大化: {}, 已应用: {})",
        geom.x,
        geom.y,
        geom.width,
        geom.height,
        geom.is_maximized,
        !still_missing_placement
    );

    !still_missing_placement
}

pub(super) static PLACEMENT_APPLIED: AtomicBool = AtomicBool::new(false);

/// Ensure the restored window placement has been applied once during the application lifecycle.
///
/// Returns `true` if placement was already applied or was successfully applied now.
/// Subsequent invocations (e.g. repeated tray hide/show cycles) are no-ops to avoid
/// corrupting the active OS window geometry or overwriting live maximized/restored states.
pub fn ensure_restored_window_placement(
    window: &slint::Window,
    base_dir: &Path,
    current_maximized: bool,
) -> bool {
    if PLACEMENT_APPLIED.load(Ordering::SeqCst) {
        return true;
    }
    if apply_restored_window_placement(window, base_dir, current_maximized) {
        if window.is_visible() {
            PLACEMENT_APPLIED.store(true, Ordering::SeqCst);
        }
        true
    } else {
        false
    }
}

#[cfg(test)]
pub fn reset_placement_applied_for_test() {
    PLACEMENT_APPLIED.store(false, Ordering::SeqCst);
}

/// The Win32 HWND behind a Slint window, when it exists.
#[cfg(windows)]
pub(crate) fn win32_window_handle(window: &slint::Window) -> Option<HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    // `WindowHandle` owns the wrapper, so bind it before borrowing the raw one.
    let win_handle = window.window_handle();
    let handle = win_handle.window_handle().ok()?;
    // Copy the pointer out: `handle` borrows `win_handle`, which is dropped here.
    let raw = match handle.as_raw() {
        RawWindowHandle::Win32(h) => h.hwnd.get() as *mut std::ffi::c_void,
        _ => return None,
    };
    Some(HWND(raw))
}

/// Persist current window geometry to disk.
///
/// Windows reads it from `GetWindowPlacement` so a maximized window still saves
/// its *restorable* size; elsewhere Slint's `Window::size`/`is_maximized` are
/// used, which the winit backends keep at the pre-maximize size.
pub fn save_current_window_geometry(window: &slint::Window, base_dir: &Path) {
    #[cfg(windows)]
    {
        if let Some(hwnd) = win32_window_handle(window)
            && let Some(geom) = capture_window_geometry(hwnd)
        {
            save_window_geometry(base_dir, &geom);
        }
    }

    #[cfg(not(windows))]
    {
        let size = window.size();
        if size.width < MIN_WINDOW_WIDTH || size.height < MIN_WINDOW_HEIGHT {
            return;
        }
        let position = window.position();
        save_window_geometry(
            base_dir,
            &WindowGeometry {
                x: position.x,
                y: position.y,
                width: size.width,
                height: size.height,
                is_maximized: window.is_maximized(),
            },
        );
    }
}
