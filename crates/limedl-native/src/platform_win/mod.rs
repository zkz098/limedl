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

#[cfg(windows)]
use parking_lot::Mutex;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::watch;

static WINDOW_VISIBLE: AtomicBool = AtomicBool::new(true);
static VISIBILITY_TX: LazyLock<watch::Sender<bool>> = LazyLock::new(|| {
    let (tx, _) = watch::channel(true);
    tx
});

/// Track whether the primary window is actively visible and not minimized.
/// Used to suppress expensive background UI model rebuilds and periodic status polling.
pub fn is_window_visible() -> bool {
    WINDOW_VISIBLE.load(Ordering::Relaxed)
}

/// Update the primary window visibility state.
pub fn set_window_visible(visible: bool) {
    let prev = WINDOW_VISIBLE.swap(visible, Ordering::SeqCst);
    if prev != visible {
        let _ = VISIBILITY_TX.send(visible);
    }
}

/// Subscribe to primary window visibility changes.
pub fn subscribe_window_visibility() -> watch::Receiver<bool> {
    VISIBILITY_TX.subscribe()
}

/// Trim the process working set to release physical memory back to the OS.
/// Typically called when the window is minimized or hidden in the tray.
pub fn trim_working_set() {
    // 1. Instruct mimalloc to release thread caches and decommit unused pages.
    unsafe extern "C" {
        fn mi_collect(force: bool);
    }
    unsafe {
        mi_collect(true);
    }

    // 3. On Linux glibc, also instruct the system malloc to release free arenas.
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        unsafe extern "C" {
            fn malloc_trim(pad: usize) -> std::ffi::c_int;
        }
        unsafe {
            let _ = malloc_trim(0);
        }
    }

    // 4. On Windows, trim working set pages into the standby list.
    #[cfg(windows)]
    {
        use windows::Win32::System::Threading::{GetCurrentProcess, SetProcessWorkingSetSize};
        unsafe {
            let _ = SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
        }
    }
}

mod geometry;
mod os_info;

pub use geometry::*;
pub use os_info::*;

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
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
#[cfg(windows)]
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
#[cfg(windows)]
use windows::Win32::UI::Shell::{
    DefSubclassProc, DragAcceptFiles, DragFinish, DragQueryFileW, HDROP, RemoveWindowSubclass,
    SetWindowSubclass,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    IsIconic, SIZE_MAXIMIZED, SIZE_MINIMIZED, SIZE_RESTORED, SW_HIDE, SW_RESTORE,
    SetForegroundWindow, ShowWindow, WM_COPYDATA, WM_DROPFILES, WM_EXITSIZEMOVE, WM_SHOWWINDOW,
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
            DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE,
            DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
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
        WM_DROPFILES => on_dropfiles(hwnd, wparam),
        WM_SHOWWINDOW => on_showwindow(hwnd, msg, wparam, lparam),
        WM_COPYDATA => on_copydata(hwnd, msg, wparam, lparam),
        WM_EXITSIZEMOVE => {
            save_geometry_if_configured(hwnd);
            unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
        }
        WM_SIZE => on_size(hwnd, msg, wparam, lparam),
        _ => unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) },
    }
}

/// `WM_DROPFILES`: collect the dropped paths and hand them to the callback.
#[cfg(windows)]
fn on_dropfiles(hwnd: HWND, wparam: WPARAM) -> LRESULT {
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

    unsafe { DragFinish(hdrop) };

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

/// `WM_SHOWWINDOW`: track visibility and run the show callback.
#[cfg(windows)]
fn on_showwindow(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if wparam.0 != 0 {
        set_window_visible(true);
        if let Some(ref cb) = *SHOW_CALLBACK.lock() {
            cb();
        }
    } else {
        set_window_visible(false);
        trim_working_set();
    }
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

/// `WM_COPYDATA`: decode the single-instance payload and run the callback.
///
/// Returns `LRESULT(1)` when the payload was consumed, otherwise falls through
/// to `DefSubclassProc`.
#[cfg(windows)]
fn on_copydata(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if lparam.0 != 0 {
        let cds = unsafe { &*(lparam.0 as *const COPYDATASTRUCT) };
        if cds.dwData == COPYDATA_MAGIC {
            let payload = read_copydata_payload(cds);

            unsafe {
                let _ = ShowWindow(hwnd, SW_RESTORE);
                let _ = SetForegroundWindow(hwnd);
            }
            set_window_visible(true);

            if let Some(ref cb) = *COPYDATA_CALLBACK.lock() {
                cb(payload);
            }
            return LRESULT(1);
        }
    }
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

/// Decode a `WM_COPYDATA` payload as UTF-8 text (empty payload → `None`).
#[cfg(windows)]
fn read_copydata_payload(cds: &COPYDATASTRUCT) -> Option<String> {
    if cds.cbData == 0 || cds.lpData.is_null() {
        return None;
    }
    let slice = unsafe { std::slice::from_raw_parts(cds.lpData as *const u8, cds.cbData as usize) };
    let text = String::from_utf8_lossy(slice).trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

/// `WM_SIZE`: visibility and geometry updates for a size change.
#[cfg(windows)]
fn on_size(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let size_type = wparam.0 as u32;
    if size_type == SIZE_MAXIMIZED || size_type == SIZE_RESTORED {
        set_window_visible(true);
        save_geometry_if_configured(hwnd);
    } else if size_type == SIZE_MINIMIZED {
        set_window_visible(false);
        trim_working_set();
    }
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

/// Restore and bring the window to the foreground via Win32 APIs.
pub fn bring_to_foreground(window: &slint::Window) {
    #[cfg(windows)]
    {
        if let Some(hwnd) = win32_window_handle(window) {
            unsafe {
                if IsIconic(hwnd).as_bool() {
                    let _ = ShowWindow(hwnd, SW_RESTORE);
                }
                let _ = SetForegroundWindow(hwnd);
            }
        }
    }

    #[cfg(not(windows))]
    {
        let _ = window;
    }
}

/// Explicitly hide the native OS window.
pub fn hide_window(window: &slint::Window) {
    set_window_visible(false);
    trim_working_set();
    #[cfg(windows)]
    {
        if let Some(hwnd) = win32_window_handle(window) {
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
        }
    }

    #[cfg(not(windows))]
    {
        let _ = window;
    }
}

/// Remove hooks on teardown.
#[allow(dead_code)]
pub fn cleanup_window_hooks(window: &slint::Window) {
    #[cfg(windows)]
    {
        if !HOOK_INSTALLED.swap(false, Ordering::SeqCst) {
            return;
        }

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
mod tests;
