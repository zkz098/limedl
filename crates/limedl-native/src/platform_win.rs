//! Windows platform native integrations for limedl-native:
//! - Window drag-and-drop support (`WM_DROPFILES` via `DragAcceptFiles` + `SetWindowSubclass`).
//! - Secondary instance inter-process activation & argument passing (`WM_COPYDATA`).

use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

type DropCallback = Box<dyn Fn(Vec<String>) + Send + Sync + 'static>;
type CopyDataCallback = Box<dyn Fn(String) + Send + Sync + 'static>;

static DROP_CALLBACK: Mutex<Option<DropCallback>> = Mutex::new(None);
static COPYDATA_CALLBACK: Mutex<Option<CopyDataCallback>> = Mutex::new(None);
static HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);

const SUBCLASS_ID: usize = 0x4C494D45; // "LIME"
pub const COPYDATA_MAGIC: usize = 0x4C494D45;

#[cfg(windows)]
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
#[cfg(windows)]
use windows::Win32::UI::Shell::{
    DefSubclassProc, DragAcceptFiles, DragFinish, DragQueryFileW, HDROP, RemoveWindowSubclass,
    SetWindowSubclass,
};
#[cfg(windows)]
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    SW_RESTORE, SetForegroundWindow, ShowWindow, WM_COPYDATA, WM_DROPFILES,
};

/// Store the drag-drop / IPC callbacks without touching the OS window.
///
/// Safe to call before the native window exists; `try_install_window_hooks`
/// then performs the Win32 part once the handle is available.
pub fn set_callbacks(
    on_drop: impl Fn(Vec<String>) + Send + Sync + 'static,
    on_copydata: impl Fn(String) + Send + Sync + 'static,
) {
    *DROP_CALLBACK.lock() = Some(Box::new(on_drop));
    *COPYDATA_CALLBACK.lock() = Some(Box::new(on_copydata));
}

/// Install the Win32 hooks (drag-and-drop + WM_COPYDATA) on the Slint window.
///
/// Returns `false` while the OS window does not exist yet (Slint creates it when
/// the event loop starts), so callers can retry on a timer instead of silently
/// losing the integrations. Callbacks are registered separately via
/// [`set_callbacks`].
pub fn try_install_window_hooks(window: &slint::Window) -> bool {
    #[cfg(windows)]
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

    #[cfg(not(windows))]
    {
        let _ = window;
        false
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
        WM_COPYDATA => {
            if lparam.0 != 0 {
                let cds = unsafe { &*(lparam.0 as *const COPYDATASTRUCT) };
                if cds.dwData == COPYDATA_MAGIC && cds.cbData > 0 && !cds.lpData.is_null() {
                    let slice = unsafe {
                        std::slice::from_raw_parts(cds.lpData as *const u8, cds.cbData as usize)
                    };
                    let text = String::from_utf8_lossy(slice).to_string();

                    unsafe {
                        let _ = ShowWindow(hwnd, SW_RESTORE);
                        let _ = SetForegroundWindow(hwnd);
                    }

                    if let Some(ref cb) = *COPYDATA_CALLBACK.lock() {
                        cb(text);
                    }
                    return LRESULT(1);
                }
            }
            unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
        }
        _ => unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) },
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
