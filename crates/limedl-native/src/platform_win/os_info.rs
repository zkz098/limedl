/// Friendly OS description for the About tab (e.g. "Windows 11 Pro (build 22631)").
///
/// Windows keeps the marketing name + build number in the registry; Windows 11
/// still reports `ProductName = Windows 10 …` in some builds, so the build
/// number decides. Other platforms fall back to the Rust target OS name.
pub fn os_description() -> String {
    #[cfg(windows)]
    {
        if let Ok(key) =
            windows_registry::LOCAL_MACHINE.open(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
        {
            let product = key
                .get_string("ProductName")
                .unwrap_or_else(|_| "Windows".to_string());
            let build = key
                .get_string("CurrentBuildNumber")
                .or_else(|_| key.get_string("CurrentBuild"))
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
