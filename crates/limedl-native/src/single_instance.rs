//! Single-instance guard for limedl-native.
//!
//! Behaviour mirrors the single-instance semantics of the previous desktop
//! shell: a second launch activates (shows + foregrounds) the existing window
//! and exits immediately instead of starting a second engine instance.
//!
//! Platform strategies:
//! - **Windows**: a session-local named mutex (`Local\...`) claims the instance;
//!   secondary launches focus the existing window by title via `FindWindowW` and
//!   forward the CLI payload with `WM_COPYDATA`.
//! - **macOS / Linux**: an exclusive advisory lock on `<data-dir>/instance.lock`
//!   claims the instance, and the primary publishes a Unix domain socket next to
//!   it that secondary launches use to wake it up. The kernel drops the lock
//!   when the process dies, so a crash cannot leave a stale owner behind.
//!
//! The non-Windows claim used to bind a fixed loopback TCP port instead. Any
//! unrelated program that happened to own that port then looked like a running
//! limedl: the launch was classified secondary, the "notify the primary" connect
//! failed silently, and the user got neither a window nor an error. A lock file
//! cannot be spoofed by an unrelated listener, and
//! [`InstanceClaim::notify_primary`] now reports failure so the caller can
//! surface it.

use std::path::{Path, PathBuf};

#[cfg(windows)]
const MUTEX_NAME: &str = "Local\\limedl-native-single-instance";
#[cfg(windows)]
const WINDOW_TITLE: &str = "limedl - Native";

/// Unix socket paths are length-capped (`sun_path` is 104 bytes on macOS, 108 on
/// Linux). A data directory deeper than this budget falls back to a short
/// hashed name under the temp directory.
#[cfg(not(windows))]
const UNIX_SOCKET_PATH_BUDGET: usize = 100;

/// How long a secondary keeps retrying the activation socket before giving up.
/// The primary binds it a few milliseconds into startup, so the only reason to
/// retry is the "double-clicked twice" race with a starting primary.
#[cfg(not(windows))]
const ACTIVATION_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);
#[cfg(not(windows))]
const ACTIVATION_CONNECT_INTERVAL: std::time::Duration = std::time::Duration::from_millis(50);

/// Result of the single-instance claim at startup.
pub struct InstanceClaim {
    /// The data directory the claim belongs to. Two instances with different
    /// data directories are independent and may both run. Read on Unix to locate
    /// the activation socket; on Windows the mutex name is global and the payload
    /// travels through `WM_COPYDATA`, so it is only kept for symmetry.
    #[cfg_attr(windows, allow(dead_code))]
    base_dir: PathBuf,
    state: ClaimState,
}

enum ClaimState {
    /// This process owns the app instance. `PrimaryHandle` keeps the claim alive
    /// (mutex handle / file lock) for the process lifetime. On Windows the
    /// handle is only *held* (the mutex lives until process exit) and never read
    /// again, so the field is intentionally exempt from `dead_code`.
    Primary(#[cfg_attr(windows, allow(dead_code))] PrimaryHandle),
    /// Another instance is already running.
    Secondary,
}

/// Keeps the primary claim alive. Dropping the inner resource would release the
/// claim, so it must live until process exit (intentional leak-by-hold).
pub enum PrimaryHandle {
    #[cfg(windows)]
    // The raw HANDLE is never read again, but it must NOT be closed: keeping
    // it open holds the mutex for the process lifetime.
    #[allow(dead_code)]
    Windows(windows::Win32::Foundation::HANDLE),
    #[cfg(not(windows))]
    Unix {
        /// Held only for its lock: the kernel releases the exclusive advisory
        /// lock when this handle closes (process exit), never earlier. `None`
        /// only when the lock file could not be created at all.
        #[allow(dead_code)]
        lock: Option<std::fs::File>,
        /// `None` when the activation socket could not be bound (path too long,
        /// unexpected I/O error). The instance is still the primary; only the
        /// "a second launch wakes me up" convenience is lost.
        listener: Option<std::os::unix::net::UnixListener>,
    },
}

impl InstanceClaim {
    /// Try to become the primary instance for `base_dir`.
    pub fn claim(base_dir: &Path) -> Self {
        let state = claim_state(base_dir);
        Self {
            base_dir: base_dir.to_path_buf(),
            state,
        }
    }

    pub fn is_secondary(&self) -> bool {
        matches!(self.state, ClaimState::Secondary)
    }

    /// Called by a secondary instance right before exiting: activate the primary
    /// instance's window and forward an optional argument (magnet link, torrent
    /// path, …).
    ///
    /// Returns whether the request reached a primary. `false` means the caller
    /// must not exit quietly: something holds the instance claim but is not
    /// reachable, and the user has to be told.
    pub fn notify_primary(&self, payload: Option<&str>) -> bool {
        #[cfg(windows)]
        {
            notify_primary_windows(payload)
        }

        #[cfg(not(windows))]
        {
            notify_primary_unix(&self.base_dir, payload)
        }
    }

    /// Called by the primary instance: start handling activate requests from
    /// secondary launches (no-op on Windows, where secondary instances focus the
    /// window directly and send `WM_COPYDATA`). `activate` runs on the listener
    /// thread and must marshal onto the Slint UI thread via
    /// `invoke_from_event_loop`.
    pub fn listen_for_activate(&self, activate: impl Fn(Option<String>) + Send + 'static) {
        #[cfg(not(windows))]
        {
            let ClaimState::Primary(PrimaryHandle::Unix {
                listener: Some(listener),
                ..
            }) = &self.state
            else {
                return;
            };
            let listener = match listener.try_clone() {
                Ok(listener) => listener,
                Err(error) => {
                    tracing::warn!("could not clone the activation socket listener: {error}");
                    return;
                }
            };
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let mut stream = match stream {
                        Ok(stream) => stream,
                        Err(_) => continue,
                    };
                    let mut buf = String::new();
                    let _ = std::io::Read::read_to_string(&mut stream, &mut buf);
                    let payload = buf
                        .strip_prefix("open:")
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty());
                    activate(payload);
                }
            });
        }
        #[cfg(windows)]
        let _ = activate;
    }
}

// ---------------------------------------------------------------------------
// Windows
// ---------------------------------------------------------------------------

#[cfg(windows)]
fn claim_state(_base_dir: &Path) -> ClaimState {
    use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
    use windows::Win32::System::Threading::CreateMutexW;
    use windows::core::HSTRING;

    match unsafe { CreateMutexW(None, false, &HSTRING::from(MUTEX_NAME)) } {
        // NOTE: when the mutex already exists CreateMutexW still SUCCEEDS and
        // returns a handle to it — ownership is signalled exclusively via
        // GetLastError() == ERROR_ALREADY_EXISTS.
        // HANDLE has no Drop in the windows crate; keeping the value (never
        // CloseHandle) holds the mutex until process exit.
        Ok(handle) => {
            if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
                ClaimState::Secondary
            } else {
                ClaimState::Primary(PrimaryHandle::Windows(handle))
            }
        }
        // On unexpected API failure degrade to primary so a broken mutex
        // namespace can never brick the app.
        Err(err) => {
            eprintln!("[limedl] single-instance mutex error: {err}");
            ClaimState::Primary(PrimaryHandle::Windows(
                windows::Win32::Foundation::HANDLE::default(),
            ))
        }
    }
}

#[cfg(windows)]
fn notify_primary_windows(payload: Option<&str>) -> bool {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::System::DataExchange::COPYDATASTRUCT;
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, SW_RESTORE, SendMessageW, SetForegroundWindow, ShowWindow, WM_COPYDATA,
    };
    use windows::core::HSTRING;

    // Best effort by design: the window may not exist yet while the primary is
    // still starting, and in that case the primary is about to show it anyway.
    unsafe {
        let Ok(hwnd) = FindWindowW(None, &HSTRING::from(WINDOW_TITLE)) else {
            return true;
        };
        // SW_RESTORE shows a hidden (tray-minimized) window and restores a
        // minimized one; a normal window stays normal.
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let _ = SetForegroundWindow(hwnd);

        // Always send WM_COPYDATA so the primary instance can restore,
        // un-minimize and request redraw via Slint's event loop.
        let text = payload.unwrap_or("");
        let bytes = text.as_bytes();
        let cds = COPYDATASTRUCT {
            dwData: crate::platform_win::COPYDATA_MAGIC,
            cbData: bytes.len() as u32,
            lpData: if bytes.is_empty() {
                std::ptr::null_mut()
            } else {
                bytes.as_ptr() as *mut std::ffi::c_void
            },
        };
        let _ = SendMessageW(
            hwnd,
            WM_COPYDATA,
            Some(WPARAM(0)),
            Some(LPARAM(&cds as *const _ as isize)),
        );
    }
    true
}

// ---------------------------------------------------------------------------
// macOS / Linux
// ---------------------------------------------------------------------------

/// Outcome of trying to take the instance lock.
#[cfg(not(windows))]
enum LockAttempt {
    /// The lock is ours; the file handle keeps it held.
    Acquired(std::fs::File),
    /// The lock file could not even be opened. Stay primary instead of refusing
    /// to start (the same policy as the Windows mutex path).
    Unavailable,
    /// Another live process holds the lock.
    Held,
}

#[cfg(not(windows))]
fn claim_state(base_dir: &Path) -> ClaimState {
    match try_lock_instance(base_dir) {
        LockAttempt::Held => ClaimState::Secondary,
        LockAttempt::Unavailable => ClaimState::Primary(PrimaryHandle::Unix {
            lock: None,
            listener: None,
        }),
        LockAttempt::Acquired(lock) => {
            let socket_path = activation_socket_path(base_dir);
            let listener = bind_activation_socket(&socket_path);
            ClaimState::Primary(PrimaryHandle::Unix {
                lock: Some(lock),
                listener,
            })
        }
    }
}

/// Take the exclusive advisory lock at `<base_dir>/instance.lock`.
#[cfg(not(windows))]
fn try_lock_instance(base_dir: &Path) -> LockAttempt {
    let path = base_dir.join("instance.lock");
    if let Some(parent) = path.parent()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        eprintln!(
            "[limedl] single-instance lock directory {} unavailable: {error}",
            parent.display()
        );
        return LockAttempt::Unavailable;
    }

    let file = match std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&path)
    {
        Ok(file) => file,
        Err(error) => {
            eprintln!(
                "[limedl] single-instance lock {} unavailable: {error}",
                path.display()
            );
            return LockAttempt::Unavailable;
        }
    };

    // `try_lock` is the std file lock: the lock lives on the open file
    // description, so the kernel releases it when this process exits — crashed
    // or not — and a second descriptor in the same process is rejected too.
    match file.try_lock() {
        Ok(()) => LockAttempt::Acquired(file),
        Err(_) => LockAttempt::Held,
    }
}

/// Where the activation socket lives. Deep data directories fall back to a
/// short, deterministic temp-directory name because `sun_path` is length-capped.
#[cfg(not(windows))]
fn activation_socket_path(base_dir: &Path) -> PathBuf {
    use std::hash::{Hash as _, Hasher as _};

    let direct = base_dir.join("instance.sock");
    if direct.as_os_str().len() <= UNIX_SOCKET_PATH_BUDGET {
        return direct;
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    base_dir.hash(&mut hasher);
    std::env::temp_dir().join(format!("limedl-{:016x}.sock", hasher.finish()))
}

/// Bind the activation socket, replacing a leftover file from a crashed primary.
///
/// Removing the stale path is safe because the caller already holds the instance
/// lock: no live primary can own it.
#[cfg(not(windows))]
fn bind_activation_socket(path: &Path) -> Option<std::os::unix::net::UnixListener> {
    let _ = std::fs::remove_file(path);
    match std::os::unix::net::UnixListener::bind(path) {
        Ok(listener) => Some(listener),
        Err(error) => {
            tracing::warn!(
                "could not bind the activation socket {}: {error} — a second launch will \
                 report that this instance is unreachable",
                path.display()
            );
            None
        }
    }
}

#[cfg(not(windows))]
fn notify_primary_unix(base_dir: &Path, payload: Option<&str>) -> bool {
    notify_primary_unix_within(base_dir, payload, ACTIVATION_CONNECT_TIMEOUT)
}

#[cfg(not(windows))]
fn notify_primary_unix_within(
    base_dir: &Path,
    payload: Option<&str>,
    timeout: std::time::Duration,
) -> bool {
    use std::io::Write as _;

    let path = activation_socket_path(base_dir);
    let deadline = std::time::Instant::now() + timeout;
    let msg = match payload {
        Some(payload) => format!("open:{payload}\n"),
        None => "show\n".to_string(),
    };

    loop {
        match std::os::unix::net::UnixStream::connect(&path) {
            Ok(mut stream) => {
                let sent = stream
                    .write_all(msg.as_bytes())
                    .and_then(|()| stream.flush())
                    .and_then(|()| stream.shutdown(std::net::Shutdown::Write));
                if let Err(error) = sent {
                    tracing::warn!("activation request to {} failed: {error}", path.display());
                    return false;
                }
                return true;
            }
            Err(error) => {
                if std::time::Instant::now() >= deadline {
                    tracing::warn!(
                        "could not reach the running limedl instance at {}: {error}",
                        path.display()
                    );
                    return false;
                }
                // The primary may still be starting up; retry briefly.
                std::thread::sleep(ACTIVATION_CONNECT_INTERVAL);
            }
        }
    }
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn a_second_claim_on_the_same_data_dir_is_secondary() {
        let dir = tempfile::tempdir().expect("tempdir");
        let first = InstanceClaim::claim(dir.path());
        assert!(!first.is_secondary(), "the first claim must win");

        let second = InstanceClaim::claim(dir.path());
        assert!(second.is_secondary(), "the second claim must lose");
    }

    #[test]
    fn claims_on_different_data_dirs_are_independent() {
        let a = tempfile::tempdir().expect("tempdir");
        let b = tempfile::tempdir().expect("tempdir");
        assert!(!InstanceClaim::claim(a.path()).is_secondary());
        assert!(!InstanceClaim::claim(b.path()).is_secondary());
    }

    #[test]
    fn secondary_delivers_the_cli_payload_to_the_primary() {
        let dir = tempfile::tempdir().expect("tempdir");
        let primary = InstanceClaim::claim(dir.path());
        let (tx, rx) = mpsc::channel();
        primary.listen_for_activate(move |payload| {
            let _ = tx.send(payload);
        });

        let secondary = InstanceClaim::claim(dir.path());
        assert!(secondary.is_secondary());
        assert!(
            secondary.notify_primary(Some("magnet:?xt=urn:btih:abc")),
            "the primary must accept the activation request"
        );

        let received = rx
            .recv_timeout(Duration::from_secs(10))
            .expect("the primary must receive the payload");
        assert_eq!(received.as_deref(), Some("magnet:?xt=urn:btih:abc"));
    }

    #[test]
    fn secondary_reaches_a_primary_that_is_still_starting_up() {
        let dir = tempfile::tempdir().expect("tempdir");
        let primary = InstanceClaim::claim(dir.path());

        // The primary has claimed and bound the socket but is not accepting yet —
        // the state a double-click while the first launch boots produces.
        let secondary = InstanceClaim::claim(dir.path());
        assert!(secondary.is_secondary());
        assert!(
            secondary.notify_primary(Some("https://example.com/a.zip")),
            "the request must be queued, not refused"
        );

        let (tx, rx) = mpsc::channel();
        primary.listen_for_activate(move |payload| {
            let _ = tx.send(payload);
        });
        let received = rx
            .recv_timeout(Duration::from_secs(10))
            .expect("the queued payload must be delivered once the primary accepts");
        assert_eq!(received.as_deref(), Some("https://example.com/a.zip"));
    }

    #[test]
    fn unreachable_primary_is_reported_instead_of_silenced() {
        let dir = tempfile::tempdir().expect("tempdir");
        // Nothing claimed this directory, so no socket was ever bound.
        assert!(!notify_primary_unix_within(
            dir.path(),
            None,
            Duration::ZERO
        ));
    }

    #[test]
    fn activation_socket_stays_within_the_sun_path_budget() {
        let short = tempfile::tempdir().expect("tempdir");
        assert!(activation_socket_path(short.path()).starts_with(short.path()));

        let deep = Path::new("/tmp").join("d".repeat(120));
        let fallback = activation_socket_path(&deep);
        assert!(
            fallback.as_os_str().len() <= UNIX_SOCKET_PATH_BUDGET,
            "fallback path is still too long: {}",
            fallback.display()
        );
    }
}
