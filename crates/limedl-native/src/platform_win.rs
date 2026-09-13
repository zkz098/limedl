//! Platform native integrations for limedl-native.
//!
//! Windows (`#[cfg(windows)]`): window drag-and-drop (`WM_DROPFILES` via
//! `DragAcceptFiles` + `SetWindowSubclass`), secondary-instance activation
//! (`WM_COPYDATA`), DWM title-bar theming and monitor-aware window placement.
//!
//! Every platform: persisted window geometry (position / size / maximized),
//! [`os_description`] and [`launched_at_logon`]. On Windows the geometry path
//! talks to Win32 (`GetWindowPlacement`/`SetWindowPos`) because that reports the
//! *unmaximized* position and detects an off-screen restore rect against the
//! real monitor list; everywhere else it goes through Slint's own
//! `Window::position`/`set_position`/`size`/`set_size`/`set_maximized` (winit),
//! which is what all three backends support uniformly.

use parking_lot::Mutex;
use std::path::{Path, PathBuf};
// Windows-only: both are used solely by the Win32 subclassing machinery below
// (`HOOK_INSTALLED`), which is compiled out everywhere else.
#[cfg(windows)]
use std::sync::atomic::{AtomicBool, Ordering};

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
const MIN_WINDOW_WIDTH: u32 = 600;
const MIN_WINDOW_HEIGHT: u32 = 400;

pub const WINDOW_STATE_FILE: &str = "window_state.json";
static BASE_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);
static LAST_SAVED_GEOMETRY: Mutex<Option<WindowGeometry>> = Mutex::new(None);

/// Configure base directory for persisting window geometry.
pub fn set_base_dir(dir: PathBuf) {
    *BASE_DIR.lock() = Some(dir);
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

// ── Windows-only: drag-and-drop + WM_COPYDATA IPC ───────────────────────────
// Everything from here to the `subclass_proc` handler exists to pass file drops
// and secondary-launch payloads into the app from the Win32 message loop, and is
// reachable only through `#[cfg(windows)]` call sites (main.rs, single_instance.rs).
// The whole set has to be gated, not just the functions: on macOS/Linux the
// callbacks and ids have no readers, and `-D warnings` turns each unused type
// alias, static and const into a build failure (the earlier unconditional
// definitions compiled fine on Windows for exactly that reason).
#[cfg(windows)]
type DropCallback = Box<dyn Fn(Vec<String>) + Send + Sync + 'static>;
#[cfg(windows)]
type CopyDataCallback = Box<dyn Fn(Option<String>) + Send + Sync + 'static>;
#[cfg(windows)]
type ShowCallback = Box<dyn Fn() + Send + Sync + 'static>;

#[cfg(windows)]
static DROP_CALLBACK: Mutex<Option<DropCallback>> = Mutex::new(None);
#[cfg(windows)]
static COPYDATA_CALLBACK: Mutex<Option<CopyDataCallback>> = Mutex::new(None);
#[cfg(windows)]
static SHOW_CALLBACK: Mutex<Option<ShowCallback>> = Mutex::new(None);
#[cfg(windows)]
static HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
const SUBCLASS_ID: usize = 0x4C494D45; // "LIME"
/// `dwData` magic marking a `WM_COPYDATA` sent by another limedl instance
/// (read by `single_instance::notify_primary`).
#[cfg(windows)]
pub const COPYDATA_MAGIC: usize = 0x4C494D45;

#[cfg(windows)]
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
#[cfg(windows)]
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromRect, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONULL,
    MONITOR_DEFAULTTOPRIMARY,
};
#[cfg(windows)]
use windows::Win32::UI::Shell::{
    DefSubclassProc, DragAcceptFiles, DragFinish, DragQueryFileW, HDROP, RemoveWindowSubclass,
    SetWindowSubclass,
};
#[cfg(windows)]
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowPlacement, GetWindowRect, IsZoomed, SetForegroundWindow, SetWindowPos, ShowWindow,
    SIZE_MAXIMIZED, SIZE_RESTORED, SW_MAXIMIZE, SW_RESTORE, SWP_NOACTIVATE, SWP_NOSIZE,
    SWP_NOZORDER, WINDOWPLACEMENT, WM_COPYDATA, WM_DROPFILES, WM_EXITSIZEMOVE, WM_SHOWWINDOW,
    WM_SIZE,
};

/// Store the drag-drop / IPC callbacks without touching the OS window.
///
/// Safe to call before the native window exists; `try_install_window_hooks`
/// then performs the Win32 part once the handle is available.
///
/// Windows-only, like every other export in this group — the whole drag-drop and
/// `WM_COPYDATA` path has no non-Windows equivalent (`main.rs` calls it from a
/// `#[cfg(windows)]` block), so there is nothing to no-op elsewhere.
#[cfg(windows)]
pub fn set_callbacks(
    on_drop: impl Fn(Vec<String>) + Send + Sync + 'static,
    on_copydata: impl Fn(Option<String>) + Send + Sync + 'static,
    on_show: impl Fn() + Send + Sync + 'static,
) {
    *DROP_CALLBACK.lock() = Some(Box::new(on_drop));
    *COPYDATA_CALLBACK.lock() = Some(Box::new(on_copydata));
    *SHOW_CALLBACK.lock() = Some(Box::new(on_show));
}

/// Install the Win32 hooks (drag-and-drop + WM_COPYDATA) on the Slint window.
///
/// Returns `false` while the OS window does not exist yet (Slint creates it when
/// the event loop starts), so callers can retry on a timer instead of silently
/// losing the integrations. Callbacks are registered separately via
/// [`set_callbacks`].
#[cfg(windows)]
pub fn try_install_window_hooks(window: &slint::Window) -> bool {
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};

        let win_handle = window.window_handle();
        let Ok(handle_wrapper) = win_handle.window_handle() else {
            tracing::debug!("原生窗口句柄尚未就绪，稍后重试挂载");
            return false;
        };

        let hwnd_raw = match handle_wrapper.as_raw() {
            RawWindowHandle::Win32(h) => h.hwnd.get() as *mut std::ffi::c_void,
            _ => return false,
        };

        let hwnd = HWND(hwnd_raw);

        unsafe {
            // Enable native shell file drag-drop
            DragAcceptFiles(hwnd, true);

            // Subclass the window to intercept WM_DROPFILES & WM_COPYDATA
            let ok = SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, 0);
            if ok.as_bool() {
                HOOK_INSTALLED.store(true, Ordering::SeqCst);
                tracing::info!("成功挂载 Windows 窗口子类化处理器 (Drag-and-Drop + WM_COPYDATA)");
                return true;
            }
            tracing::warn!("挂载 Windows 窗口子类化处理器失败");
        }
        false
    }
}

/// Synchronize the native Windows title bar appearance with the application theme:
/// - Sets `DWMWA_USE_IMMERSIVE_DARK_MODE` for Windows 10/11 title bar styling.
/// - Sets `DWMWA_CAPTION_COLOR` so the title bar matches the window's top toolbar.
/// - Sets `DWMWA_TEXT_COLOR` for crisp caption text contrast.
/// - Sets `DWMWA_WINDOW_CORNER_PREFERENCE` to ensure Windows 11 rounded corners.
pub fn sync_window_theme(window: &slint::Window, is_dark: bool) {
    #[cfg(windows)]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR,
            DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE,
            DWMWCP_ROUND,
        };

        let win_handle = window.window_handle();
        let Ok(handle_wrapper) = win_handle.window_handle() else {
            return;
        };

        let hwnd_raw = match handle_wrapper.as_raw() {
            RawWindowHandle::Win32(h) => h.hwnd.get() as *mut std::ffi::c_void,
            _ => return,
        };

        let hwnd = HWND(hwnd_raw);

        unsafe {
            // Immersive dark mode (Windows 10 1809+ / Windows 11; BOOL = i32 1/0)
            let dark_val: i32 = if is_dark { 1 } else { 0 };
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &dark_val as *const _ as _,
                std::mem::size_of::<i32>() as u32,
            );

            // Caption background color (COLORREF: 0x00BBGGRR)
            // Dark: #111419 -> R: 0x11, G: 0x14, B: 0x19 -> 0x00191411
            // Light: #f8f9fa -> R: 0xf8, G: 0xf9, B: 0xfa -> 0x00faf9f8
            let caption_color: u32 = if is_dark { 0x00191411 } else { 0x00faf9f8 };
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_CAPTION_COLOR,
                &caption_color as *const _ as _,
                std::mem::size_of::<u32>() as u32,
            );

            // Caption text color (COLORREF: 0x00BBGGRR)
            // Dark: #f3f4f6 -> R: 0xf3, G: 0xf4, B: 0xf6 -> 0x00f6f4f3
            // Light: #1f2329 -> R: 0x1f, G: 0x23, B: 0x29 -> 0x0029231f
            let text_color: u32 = if is_dark { 0x00f6f4f3 } else { 0x0029231f };
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_TEXT_COLOR,
                &text_color as *const _ as _,
                std::mem::size_of::<u32>() as u32,
            );

            // Windows 11 round corners
            let corner_pref = DWMWCP_ROUND.0 as u32;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &corner_pref as *const _ as _,
                std::mem::size_of::<u32>() as u32,
            );
        }
    }

    #[cfg(not(windows))]
    {
        let _ = window;
        let _ = is_dark;
    }
}

#[cfg(windows)]
unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uidsubclass: usize,
    _dwrefdata: usize,
) -> LRESULT {
    match msg {
        WM_DROPFILES => {
            let hdrop = HDROP(wparam.0 as *mut std::ffi::c_void);
            let count = unsafe { DragQueryFileW(hdrop, 0xFFFFFFFF, None) };
            let mut files = Vec::with_capacity(count as usize);

            for i in 0..count {
                let mut buf = [0u16; 1024];
                let len = unsafe { DragQueryFileW(hdrop, i, Some(&mut buf)) };
                if len > 0 {
                    let path = String::from_utf16_lossy(&buf[..len as usize]);
                    files.push(path);
                }
            }

            unsafe {
                DragFinish(hdrop);
            }

            if !files.is_empty() {
                unsafe {
                    let _ = ShowWindow(hwnd, SW_RESTORE);
                    let _ = SetForegroundWindow(hwnd);
                }
                if let Some(ref cb) = *DROP_CALLBACK.lock() {
                    cb(files);
                }
            }

            LRESULT(0)
        }
        WM_SHOWWINDOW => {
            if wparam.0 != 0
                && let Some(ref cb) = *SHOW_CALLBACK.lock()
            {
                cb();
            }
            unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
        }
        WM_COPYDATA => {
            if lparam.0 != 0 {
                let cds = unsafe { &*(lparam.0 as *const COPYDATASTRUCT) };
                if cds.dwData == COPYDATA_MAGIC {
                    let payload = if cds.cbData > 0 && !cds.lpData.is_null() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(cds.lpData as *const u8, cds.cbData as usize)
                        };
                        let text = String::from_utf8_lossy(slice).trim().to_string();
                        if text.is_empty() {
                            None
                        } else {
                            Some(text)
                        }
                    } else {
                        None
                    };

                    unsafe {
                        let _ = ShowWindow(hwnd, SW_RESTORE);
                        let _ = SetForegroundWindow(hwnd);
                    }

                    if let Some(ref cb) = *COPYDATA_CALLBACK.lock() {
                        cb(payload);
                    }
                    return LRESULT(1);
                }
            }
            unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
        }
        WM_EXITSIZEMOVE => {
            save_geometry_if_configured(hwnd);
            unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
        }
        WM_SIZE => {
            let size_type = wparam.0 as u32;
            if size_type == SIZE_MAXIMIZED || size_type == SIZE_RESTORED {
                save_geometry_if_configured(hwnd);
            }
            unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
        }
        _ => unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) },
    }
}

#[cfg(windows)]
fn save_geometry_if_configured(hwnd: HWND) {
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
            let is_maximized = IsZoomed(hwnd).as_bool();
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
                let win_w = rect.right - rect.left;
                let win_h = rect.bottom - rect.top;
                let work_w = work.right - work.left;
                let work_h = work.bottom - work.top;
                let x = work.left + ((work_w - win_w) / 2).max(0);
                let y = work.top + ((work_h - win_h) / 2).max(0);
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    x,
                    y,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
                tracing::info!("窗口已智能居中于显示器工作区: x={x}, y={y} (工作区 {work_w}x{work_h})");
            }
        }
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
            let _ = SetWindowPos(
                hwnd,
                None,
                geom.x,
                geom.y,
                geom.width as i32,
                geom.height as i32,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            if geom.is_maximized {
                let _ = ShowWindow(hwnd, SW_MAXIMIZE);
            }
        }
    }

    #[cfg(not(windows))]
    {
        window.set_size(slint::PhysicalSize::new(geom.width, geom.height));
        window.set_position(slint::PhysicalPosition::new(geom.x, geom.y));
        if geom.is_maximized {
            window.set_maximized(true);
        }
    }

    // On Windows `is_maximized` follows the OS window, so a maximize request that
    // was dropped before the window existed shows up here and keeps the retry
    // alive. The `is_visible` half stops the retry once the window is really up,
    // so it cannot fight a user who restores a maximized window by hand.
    let still_missing_placement = geom.is_maximized
        && !current_maximized
        && (window.is_minimized() || !window.is_visible());

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

/// The Win32 HWND behind a Slint window, when it exists.
#[cfg(windows)]
fn win32_window_handle(window: &slint::Window) -> Option<HWND> {
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

/// Restore and bring the window to the foreground via Win32 APIs.
pub fn bring_to_foreground(window: &slint::Window) {
    #[cfg(windows)]
    {
        if let Some(hwnd) = win32_window_handle(window) {
            unsafe {
                let _ = ShowWindow(hwnd, SW_RESTORE);
                let _ = SetForegroundWindow(hwnd);
            }
        }
    }

    #[cfg(not(windows))]
    {
        let _ = window;
    }
}

/// Friendly OS description for the About tab (e.g. "Windows 11 Pro (build 22631)").
///
/// Windows keeps the marketing name + build number in the registry; Windows 11
/// still reports `ProductName = Windows 10 …` in some builds, so the build
/// number decides. Other platforms fall back to the Rust target OS name.
pub fn os_description() -> String {
    #[cfg(windows)]
    {
        use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};
        use winreg::RegKey;

        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        if let Ok(key) = hklm
            .open_subkey_with_flags(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion", KEY_READ)
        {
            let product = key
                .get_value::<String, _>("ProductName")
                .unwrap_or_else(|_| "Windows".to_string());
            let build = key
                .get_value::<String, _>("CurrentBuildNumber")
                .or_else(|_| key.get_value::<String, _>("CurrentBuild"))
                .unwrap_or_default();
            let product = match build.parse::<u32>() {
                Ok(n) if n >= 22000 => product.replace("Windows 10", "Windows 11"),
                _ => product,
            };
            return if build.is_empty() {
                product
            } else {
                format!("{product} (build {build})")
            };
        }
        "Windows".to_string()
    }

    #[cfg(not(windows))]
    {
        match std::env::consts::OS {
            "macos" => "macOS".to_string(),
            "linux" => "Linux".to_string(),
            other => {
                let mut chars = other.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => other.to_string(),
                }
            }
        }
    }
}

/// Seconds since the current user's shell (`explorer.exe`) started.
///
/// Used to recognise a login-time launch of the MSIX build: `startupTask`
/// entries in `AppxManifest.xml` cannot pass `--hidden` (unlike the registry /
/// `.desktop` / plist registrations), so the app infers it from "did we start
/// right after the shell?". (`WTSQuerySessionInformation(WTSLogonTime)` would be
/// the direct answer but returns `ERROR_NOT_SUPPORTED` on current Windows.)
/// Returns `None` when the information is unavailable.
pub fn seconds_since_shell_start() -> Option<u64> {
    #[cfg(windows)]
    {
        use windows::Win32::System::Threading::{
            GetCurrentProcess, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        use windows::Win32::UI::WindowsAndMessaging::{GetShellWindow, GetWindowThreadProcessId};

        // Our own start time and the shell's, both as FILETIME.
        let own = process_start_time(unsafe { GetCurrentProcess() }, GetProcessTimes)?;
        let shell_hwnd = unsafe { GetShellWindow() };
        if shell_hwnd.is_invalid() {
            return None;
        }
        let mut shell_pid = 0u32;
        unsafe { GetWindowThreadProcessId(shell_hwnd, Some(&mut shell_pid)) };
        if shell_pid == 0 {
            return None;
        }
        let shell_handle =
            unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, shell_pid) }.ok()?;
        let shell = process_start_time(shell_handle, GetProcessTimes);
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(shell_handle);
        }
        let shell = shell?;

        // Our launch delay relative to the shell start (= logon, normally).
        own.duration_since(shell).ok().map(|d| d.as_secs())
    }

    #[cfg(not(windows))]
    {
        None
    }
}

/// Read a process creation time as a `SystemTime` (FILETIME is 100 ns ticks
/// since 1601-01-01).
#[cfg(windows)]
fn process_start_time(
    handle: windows::Win32::Foundation::HANDLE,
    get_process_times: unsafe fn(
        windows::Win32::Foundation::HANDLE,
        *mut windows::Win32::Foundation::FILETIME,
        *mut windows::Win32::Foundation::FILETIME,
        *mut windows::Win32::Foundation::FILETIME,
        *mut windows::Win32::Foundation::FILETIME,
    ) -> windows::core::Result<()>,
) -> Option<std::time::SystemTime> {
    use std::time::{Duration, SystemTime};
    use windows::Win32::Foundation::FILETIME;

    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    unsafe { get_process_times(handle, &mut creation, &mut exit, &mut kernel, &mut user) }.ok()?;
    let ticks = ((creation.dwHighDateTime as u64) << 32) | creation.dwLowDateTime as u64;

    const FILETIME_TICKS_PER_SEC: u64 = 10_000_000;
    // Seconds between 1601-01-01 and the Unix epoch.
    const WINDOWS_EPOCH_OFFSET_SECS: u64 = 11_644_473_600;
    let unix_secs = ticks
        .checked_div(FILETIME_TICKS_PER_SEC)?
        .checked_sub(WINDOWS_EPOCH_OFFSET_SECS)?;
    SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs(unix_secs))
}

/// True when this process was started shortly after the user's shell, i.e. by a
/// login autostart entry rather than by the user opening the app.
pub fn launched_at_logon(window: std::time::Duration) -> bool {
    seconds_since_shell_start().is_some_and(|secs| secs <= window.as_secs())
}

/// Remove hooks on teardown.
#[allow(dead_code)]
pub fn cleanup_window_hooks(window: &slint::Window) {
    if !HOOK_INSTALLED.swap(false, Ordering::SeqCst) {
        return;
    }

    #[cfg(windows)]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};

        let win_handle = window.window_handle();
        if let Ok(handle_wrapper) = win_handle.window_handle()
            && let RawWindowHandle::Win32(h) = handle_wrapper.as_raw()
        {
            let hwnd = HWND(h.hwnd.get() as *mut std::ffi::c_void);
            unsafe {
                let _ = RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID);
            }
        }
    }

    #[cfg(not(windows))]
    {
        let _ = window;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_description_is_non_empty() {
        assert!(!os_description().trim().is_empty());
    }

    #[test]
    fn logon_time_is_sane_when_available() {
        // The shell handle may be unavailable in exotic hosts; validate shape.
        if let Some(secs) = seconds_since_shell_start() {
            assert!(secs < 60 * 60 * 24 * 365, "implausible shell age: {secs}");
        }
        // Only a shell that started "right now" can match a zero window.
        assert!(!launched_at_logon(std::time::Duration::ZERO));
    }

    #[test]
    fn window_geometry_save_and_load_roundtrip() {
        let temp_dir = std::env::temp_dir().join(format!("limedl_test_win_geom_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let geom = WindowGeometry {
            x: 250,
            y: 180,
            width: 1280,
            height: 800,
            is_maximized: false,
        };

        save_window_geometry(&temp_dir, &geom);
        let loaded = load_window_geometry(&temp_dir);
        assert_eq!(loaded, Some(geom));

        // Test invalid/too-small dimensions are rejected
        let invalid_geom = WindowGeometry {
            x: 100,
            y: 100,
            width: 200, // too small
            height: 150,
            is_maximized: false,
        };
        save_window_geometry(&temp_dir, &invalid_geom);
        assert_eq!(load_window_geometry(&temp_dir), None);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// A geometry saved by an earlier run must survive the round trip through
    /// both gates: the load-time size filter and the off-screen guard. Only when
    /// both accept it is the saved window restored instead of the OS default.
    ///
    /// Uses (0,0), the one origin that resolves to a monitor in every layout,
    /// because CI runners are frequently single-display.
    #[test]
    fn saved_geometry_passes_the_visibility_guard() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();

        let saved = WindowGeometry {
            x: 0,
            y: 0,
            width: 1024,
            height: 720,
            is_maximized: true,
        };
        save_window_geometry(base, &saved);
        let loaded = load_window_geometry(base).expect("geometry must round trip");
        assert!(is_geometry_visible_on_any_monitor(&loaded));
    }

    /// A window that lived on a since-disconnected monitor keeps coordinates no
    /// current layout can reach; restoring them would place it off-screen.
    #[test]
    fn off_screen_geometry_is_rejected_by_the_guard() {
        let stale = WindowGeometry {
            x: 40_000,
            y: 30_000,
            width: 1024,
            height: 720,
            is_maximized: false,
        };
        assert!(!is_geometry_visible_on_any_monitor(&stale));
    }

    /// The load filter drops windows that are too small to be usable, so a
    /// degenerate saved size can never be restored. Runs on every platform.
    #[test]
    fn undersized_geometry_is_rejected_on_load() {
        let dir = tempfile::tempdir().unwrap();
        let too_small = WindowGeometry {
            x: 10,
            y: 10,
            width: MIN_WINDOW_WIDTH - 1,
            height: MIN_WINDOW_HEIGHT,
            is_maximized: false,
        };
        save_window_geometry(dir.path(), &too_small);
        assert_eq!(load_window_geometry(dir.path()), None);
    }
}
