//! Single-instance guard for limedl-native.
//!
//! Behaviour mirrors `tauri-plugin-single-instance` as used by the Tauri shell:
//! a second launch activates (shows + foregrounds) the existing window and
//! exits immediately instead of starting a second engine instance.
//!
//! Platform strategies:
//! - Windows: a session-local named mutex (`Local\...`) claims the instance;
//!   secondary launches focus the existing window by title via `FindWindowW`.
//! - Other platforms: a loopback TCP listener on a fixed port claims the
//!   instance; secondary launches send a `show` request over that socket.
//!   The port is intentionally uncommon and loopback-only.

#[cfg(windows)]
const MUTEX_NAME: &str = "Local\\limedl-native-single-instance";
#[cfg(windows)]
const WINDOW_TITLE: &str = "limedl - Native";

#[cfg(not(windows))]
const SHOW_PORT: u16 = 45997;

/// Result of the single-instance claim at startup.
pub enum InstanceClaim {
    /// This process owns the app instance. `PrimaryHandle` keeps the claim
    /// alive (mutex handle / bound listener) for the process lifetime.
    // The payload is only inspected on non-Windows (activate listener);
    // on Windows it just holds the mutex handle.
    #[allow(dead_code)]
    Primary(PrimaryHandle),
    /// Another instance is already running.
    Secondary,
}

/// Keeps the primary claim alive. Dropping the inner resource would release
/// the claim, so it must live until process exit (intentional leak-by-hold).
pub enum PrimaryHandle {
    #[cfg(windows)]
    // The raw HANDLE is never read again, but it must NOT be closed: keeping
    // it open holds the mutex for the process lifetime.
    #[allow(dead_code)]
    Windows(windows::Win32::Foundation::HANDLE),
    #[cfg(not(windows))]
    #[allow(dead_code)]
    Listener(std::net::TcpListener),
}

impl InstanceClaim {
    /// Try to become the primary instance.
    pub fn claim() -> Self {
        #[cfg(windows)]
        {
            use windows::core::HSTRING;
            use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
            use windows::Win32::System::Threading::CreateMutexW;

            match unsafe { CreateMutexW(None, false, &HSTRING::from(MUTEX_NAME)) } {
                // NOTE: when the mutex already exists CreateMutexW still
                // SUCCEEDS and returns a handle to it — ownership is signalled
                // exclusively via GetLastError() == ERROR_ALREADY_EXISTS.
                // HANDLE has no Drop in the windows crate; keeping the value
                // (never CloseHandle) holds the mutex until process exit.
                Ok(handle) => {
                    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
                        Self::Secondary
                    } else {
                        Self::Primary(PrimaryHandle::Windows(handle))
                    }
                }
                // On unexpected API failure degrade to primary so a broken
                // mutex namespace can never brick the app.
                Err(err) => {
                    eprintln!("[limedl] single-instance mutex error: {err}");
                    Self::Primary(PrimaryHandle::Windows(
                        windows::Win32::Foundation::HANDLE::default(),
                    ))
                }
            }
        }

        #[cfg(not(windows))]
        {
            match std::net::TcpListener::bind(("127.0.0.1", SHOW_PORT)) {
                Ok(listener) => Self::Primary(PrimaryHandle::Listener(listener)),
                Err(_) => Self::Secondary,
            }
        }
    }

    pub fn is_secondary(&self) -> bool {
        matches!(self, Self::Secondary)
    }

    /// Called by a secondary instance right before exiting: activate the
    /// primary instance's window.
    pub fn notify_primary(&self) {
        #[cfg(windows)]
        {
            use windows::core::HSTRING;
            use windows::Win32::UI::WindowsAndMessaging::{
                FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE,
            };

            unsafe {
                if let Ok(hwnd) = FindWindowW(None, &HSTRING::from(WINDOW_TITLE)) {
                    // SW_RESTORE shows a hidden (tray-minimized) window and
                    // restores a minimized one; a normal window stays normal.
                    let _ = ShowWindow(hwnd, SW_RESTORE);
                    let _ = SetForegroundWindow(hwnd);
                }
            }
        }

        #[cfg(not(windows))]
        {
            if let Ok(mut stream) = std::net::TcpStream::connect(("127.0.0.1", SHOW_PORT)) {
                use std::io::Write;
                let _ = stream.write_all(b"show\n");
                let _ = stream.flush();
            }
        }
    }

    /// Called by the primary instance: start handling activate requests from
    /// secondary launches (no-op on Windows, where secondary instances focus
    /// the window directly). `activate` runs on the listener thread and must
    /// marshal onto the Slint UI thread via `invoke_from_event_loop`.
    pub fn listen_for_activate(&self, activate: impl Fn() + Send + 'static) {
        #[cfg(not(windows))]
        if let Self::Primary(PrimaryHandle::Listener(listener)) = self {
            let listener = match listener.try_clone() {
                Ok(l) => l,
                Err(_) => return,
            };
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let mut stream = match stream {
                        Ok(s) => s,
                        Err(_) => continue,
                    };
                    // Drain the request; any connection means "show window".
                    let _ = std::io::Read::read_to_end(&mut stream, &mut Vec::new());
                    activate();
                }
            });
        }
        #[cfg(windows)]
        let _ = activate;
    }
}

