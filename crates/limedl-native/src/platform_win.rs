//! Windows platform native integrations for limedl-native:
//! - Window drag-and-drop support (`WM_DROPFILES` via `DragAcceptFiles` + `SetWindowSubclass`).
//! - Secondary instance inter-process activation & argument passing (`WM_COPYDATA`).
//!
//! Only `launched_at_logon` is reachable on other platforms (the MSIX startup-task
//! probe runs unconditionally); `main.rs` calls the Win32 hooks inside
//! `#[cfg(windows)]`, so their absence there is expected.
#![cfg_attr(not(windows), allow(dead_code))]

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
}
