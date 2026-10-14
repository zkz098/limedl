//! Keeping the machine awake while downloads are running.
//!
//! A sleeping machine stops transferring, so a user who queued a 40 GB download
//! and walked away comes back to a stalled task. Each desktop platform has its
//! own API for the opposite request, and all of them are wrapped behind
//! [`imp::Guard`] — acquired on construction, released on `Drop`, best effort:
//!
//! - **Windows**: `SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED |
//!   ES_AWAYMODE_REQUIRED)`. `ES_SYSTEM_REQUIRED` keeps the system (and so the
//!   network stack) running, `ES_AWAYMODE_REQUIRED` lets the display go dark
//!   while that holds.
//! - **macOS**: an IOKit power assertion of type `PreventUserIdleSystemSleep` —
//!   the idle-*system* assertion, not `PreventUserIdleDisplaySleep`, so the
//!   display may still sleep. Released with `IOPMAssertionRelease`, or when the
//!   process exits.
//! - **Linux**: `org.freedesktop.login1.Manager.Inhibit` over the system D-Bus
//!   with `what = "idle:sleep"` and `mode = "block"`. logind releases the lock
//!   when the returned pipe fd is closed, so the fd *is* the lock and the guard
//!   only has to hold it.
//!
//! None of these override an explicit user action: a privileged suspend request,
//! `shutdown -h`, or the laptop lid still wins. Neither do they cover other
//! platforms — see the no-op `imp` below.
//!
//! Since the lock is taken and released through [`PowerGuard`], the *desired*
//! state is recomputed from two inputs: the number of running downloads
//! ([`PowerGuard::update`]) and `AppSettings::power.prevent_sleep`
//! ([`PowerGuard::set_enabled`]).

use parking_lot::Mutex;

/// Acquire function, kept in a field so tests can drive the state machine
/// without a real logind connection or power assertion. See the module tests.
type AcquireFn = Box<dyn Fn() -> Result<imp::Guard, String> + Send + Sync>;

/// Everything the guard remembers, behind one mutex.
///
/// The lock is held for the whole transition (including the platform call), so
/// two threads racing on `update`/`set_enabled`/`release` cannot take the OS
/// lock twice or release it while the other still wants it.
#[derive(Debug)]
struct GuardState {
    /// Running downloads, as last reported by [`PowerGuard::update`].
    active_downloads: usize,
    /// `AppSettings::power.prevent_sleep`. Defaults to `true` so a caller that
    /// never wires the setting up keeps the pre-existing behaviour.
    enabled: bool,
    /// What the last reconcile decided: `enabled && active_downloads > 0`.
    ///
    /// Kept so a *change* of desire is the only thing that touches the platform
    /// API. Without it, every UI refresh (several per second) would re-issue
    /// `SetThreadExecutionState`, and a failing logind call would retry — and log
    /// a warning — on every frame.
    desired: bool,
    /// The held OS lock, if any. `Some` is the single source of truth for "this
    /// process is currently keeping the machine awake".
    held: Option<imp::Guard>,
}

/// System sleep inhibition keyed off the number of active downloads.
pub struct PowerGuard {
    state: Mutex<GuardState>,
    acquire: AcquireFn,
}

impl std::fmt::Debug for PowerGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PowerGuard")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl Default for PowerGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl PowerGuard {
    pub fn new() -> Self {
        Self::with_acquire(imp::Guard::acquire)
    }

    fn with_acquire(acquire: impl Fn() -> Result<imp::Guard, String> + Send + Sync + 'static) -> Self {
        Self {
            state: Mutex::new(GuardState {
                active_downloads: 0,
                enabled: true,
                desired: false,
                held: None,
            }),
            acquire: Box::new(acquire),
        }
    }

    /// Report how many downloads are running right now.
    ///
    /// Returns `true` if the OS lock changed state, i.e. it was taken or
    /// released by this call.
    pub fn update(&self, active_downloads: usize) -> bool {
        let mut state = self.state.lock();
        state.active_downloads = active_downloads;
        self.apply(&mut state)
    }

    /// Apply `AppSettings::power.prevent_sleep`.
    ///
    /// Returns `true` if the OS lock changed state. Turning the setting off
    /// releases a lock that is already held; turning it on only takes effect
    /// while downloads are running, which is exactly the promise the setting
    /// makes.
    pub fn set_enabled(&self, enabled: bool) -> bool {
        let mut state = self.state.lock();
        if state.enabled == enabled {
            return false;
        }
        state.enabled = enabled;
        self.apply(&mut state)
    }

    /// Drop the OS lock right now, ignoring the setting (application exit,
    /// factory reset). The next [`update`](Self::update) may take it again if
    /// downloads are still running.
    pub fn release(&self) -> bool {
        let mut state = self.state.lock();
        // Clear the latch too: `release` must be observable even if the caller
        // goes on to report the same download count again.
        state.desired = false;
        state.drop_held()
    }

    fn apply(&self, state: &mut GuardState) -> bool {
        let desired = state.enabled && state.active_downloads > 0;
        if desired == state.desired {
            return false;
        }
        state.desired = desired;
        if !desired {
            return state.drop_held();
        }
        match (self.acquire)() {
            Ok(guard) => {
                state.held = Some(guard);
                tracing::info!("系统电源管理: 已启用下载防休眠锁 (Keep-Awake)");
                true
            }
            Err(err) => {
                // Best effort: the download is what the user asked for, the wake
                // lock is an optimisation. Stay latched off so this is logged
                // once per rising edge instead of once per UI refresh.
                tracing::warn!("系统电源管理: 申请防休眠锁失败，下载将继续但系统可能休眠: {err}");
                false
            }
        }
    }

    /// Check if a keep-awake lock is currently held.
    #[cfg(test)]
    pub fn is_preventing(&self) -> bool {
        self.state.lock().held.is_some()
    }
}

impl GuardState {
    fn drop_held(&mut self) -> bool {
        if self.held.take().is_some() {
            tracing::info!("系统电源管理: 已释放下载防休眠锁 (Restore Normal Sleep)");
            true
        } else {
            false
        }
    }
}

impl Drop for PowerGuard {
    fn drop(&mut self) {
        self.release();
    }
}

#[cfg(windows)]
mod imp {
    use windows::Win32::System::Power::{
        ES_AWAYMODE_REQUIRED, ES_CONTINUOUS, ES_SYSTEM_REQUIRED, SetThreadExecutionState,
    };

    /// `SetThreadExecutionState` is per **thread**, not per process: the reset in
    /// `drop` only cancels the requirement taken by the thread that made it.
    ///
    /// Every `PowerGuard::update` call site runs on the Slint event-loop thread,
    /// so acquire and release stay on one thread. The `release` calls on the exit
    /// paths (tray quit, factory reset) run on a runtime worker thread instead,
    /// where the reset can miss its target — harmless, because the process is
    /// about to exit, and `Guard::drop` logs an error if that ever stops being
    /// true. A non-terminal cross-thread caller would have to move the flag onto
    /// a thread it owns.
    ///
    /// The return value is ignored on purpose: it reports the *previous* state,
    /// which is legitimately `NULL` before the first call, so it cannot be used
    /// to detect failure.
    #[derive(Debug)]
    pub struct Guard {
        /// The thread whose execution state this guard owns.
        owner: std::thread::ThreadId,
    }

    impl Guard {
        pub fn acquire() -> Result<Self, String> {
            unsafe {
                SetThreadExecutionState(
                    ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_AWAYMODE_REQUIRED,
                );
            }
            Ok(Self {
                owner: std::thread::current().id(),
            })
        }
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            if self.owner != std::thread::current().id() {
                tracing::error!(
                    "系统电源管理: 防休眠锁在非持锁线程释放，线程级标志将残留到进程退出"
                );
            }
            unsafe {
                // Back to continuous normal power management.
                SetThreadExecutionState(ES_CONTINUOUS);
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn test_guard() -> Guard {
        Guard {
            owner: std::thread::current().id(),
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use objc2_core_foundation::CFString;
    use objc2_io_kit::{
        IOPMAssertionCreateWithName, IOPMAssertionID, IOPMAssertionRelease, kIOPMAssertionLevelOn,
        kIOReturnSuccess,
    };

    /// IOKit names its assertion types as strings. This one keeps the *system*
    /// awake without keeping the display on — the closest match to the Windows
    /// away-mode flags above. (`PreventUserIdleDisplaySleep` would burn the
    /// screen; `PreventSystemSleep` would also refuse a user-requested sleep,
    /// which a download has no business doing.)
    const ASSERTION_TYPE: &str = "PreventUserIdleSystemSleep";
    /// Shown by `pmset -g assertions`, so it has to say who and why.
    const ASSERTION_NAME: &str = "limedl: active downloads";

    #[derive(Debug)]
    pub struct Guard {
        id: IOPMAssertionID,
    }

    impl Guard {
        pub fn acquire() -> Result<Self, String> {
            let assertion_type = CFString::from_static_str(ASSERTION_TYPE);
            let name = CFString::from_str(ASSERTION_NAME);
            let mut id: IOPMAssertionID = 0;
            // SAFETY: `id` is a valid out-parameter, both CFStrings outlive the
            // call, and the returned assertion id is owned by this `Guard` until
            // `Drop` hands it back to `IOPMAssertionRelease`.
            let result = unsafe {
                IOPMAssertionCreateWithName(
                    Some(&assertion_type),
                    kIOPMAssertionLevelOn,
                    Some(&name),
                    &mut id,
                )
            };
            if result != kIOReturnSuccess {
                return Err(format!("IOPMAssertionCreateWithName: {result:#010x}"));
            }
            Ok(Self { id })
        }
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            // `id` came from a successful `IOPMAssertionCreateWithName` and is
            // released exactly once, here. The binding is safe to call: the id
            // is an opaque handle, and releasing an already-released one is the
            // library's business, not UB.
            IOPMAssertionRelease(self.id);
        }
    }

    #[cfg(test)]
    pub(crate) fn test_guard() -> Guard {
        Guard { id: 0 }
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use zbus::blocking::Connection;
    use zbus::proxy;
    use zbus::zvariant::OwnedFd;

    /// "sleep" refuses suspend/hibernate requests, "idle" stops the machine from
    /// auto-suspending because nothing looks busy. Both are needed: desktops
    /// usually suspend on idle, and only the "sleep" half covers a user hitting
    /// the suspend key while a download is running. The lid switch is
    /// deliberately not inhibited.
    const WHAT: &str = "idle:sleep";
    const WHO: &str = "limedl";
    /// Shown by `systemd-inhibit --list` (and in the desktop's power dialog for
    /// a `delay` lock), so it has to say why.
    const WHY: &str = "Downloading";
    /// "block" rather than "delay": a delay lock only postpones the suspend by
    /// `InhibitDelayMaxSec`, far too short for a download to finish anyway.
    const MODE: &str = "block";

    #[proxy(
        interface = "org.freedesktop.login1.Manager",
        default_service = "org.freedesktop.login1",
        default_path = "/org/freedesktop/login1"
    )]
    trait Manager {
        /// Returns the inhibition lock's pipe fd; see the module docs.
        fn inhibit(
            &self,
            what: &str,
            who: &str,
            why: &str,
            mode: &str,
        ) -> zbus::Result<zbus::zvariant::OwnedFd>;
    }

    #[derive(Debug)]
    pub struct Guard {
        /// Closing this fd releases the inhibit, so it is the whole guard: the
        /// D-Bus connection used to create it is dropped right after the call.
        _fd: OwnedFd,
    }

    impl Guard {
        pub fn acquire() -> Result<Self, String> {
            // A fresh connection per acquire is deliberate: the lock's lifetime
            // is the fd's, so caching a connection would only add a way for a
            // stale connection to outlive the lock it made.
            let connection =
                Connection::system().map_err(|err| format!("system bus unavailable: {err}"))?;
            let manager = ManagerProxyBlocking::new(&connection)
                .map_err(|err| format!("logind proxy: {err}"))?;
            let fd: OwnedFd = manager
                .inhibit(WHAT, WHO, WHY, MODE)
                .map_err(|err| format!("logind Inhibit: {err}"))?;
            Ok(Self { _fd: fd })
        }
    }

    #[cfg(test)]
    pub(crate) fn test_guard() -> Guard {
        // Any fd will do: the state-machine tests never call logind, they only
        // need a value to hold and drop.
        let file = std::fs::File::open("/dev/null").expect("/dev/null must be openable");
        Guard {
            _fd: std::os::fd::OwnedFd::from(file).into(),
        }
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
mod imp {
    //! No keep-awake backend for this platform. The guard still flips, so the
    //! setting and the state machine behave identically — the machine's own
    //! power policy is simply left alone.

    #[derive(Debug)]
    pub struct Guard;

    impl Guard {
        pub fn acquire() -> Result<Self, String> {
            Ok(Self)
        }
    }

    #[cfg(test)]
    pub(crate) fn test_guard() -> Guard {
        Guard
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// The recordings a test needs: how often the lock was taken, and how often
    /// it was attempted. Per-guard rather than `static` so the tests stay
    /// isolated under a threaded `cargo test` as well as nextest's process per
    /// test.
    #[derive(Clone, Default)]
    struct Recorded {
        acquired: Arc<AtomicUsize>,
        attempts: Arc<AtomicUsize>,
    }

    impl Recorded {
        fn acquired(&self) -> usize {
            self.acquired.load(Ordering::SeqCst)
        }

        fn attempts(&self) -> usize {
            self.attempts.load(Ordering::SeqCst)
        }

        /// A backend that always succeeds.
        fn succeeding(&self) -> impl Fn() -> Result<imp::Guard, String> + Send + Sync + 'static {
            let recorded = self.clone();
            move || {
                recorded.attempts.fetch_add(1, Ordering::SeqCst);
                recorded.acquired.fetch_add(1, Ordering::SeqCst);
                Ok(imp::test_guard())
            }
        }

        /// A backend that fails, e.g. no logind on the system bus.
        fn failing(&self) -> impl Fn() -> Result<imp::Guard, String> + Send + Sync + 'static {
            let recorded = self.clone();
            move || {
                recorded.attempts.fetch_add(1, Ordering::SeqCst);
                Err(String::from("no logind on this bus"))
            }
        }
    }

    #[test]
    fn test_power_guard_transitions() {
        let recorded = Recorded::default();
        let guard = PowerGuard::with_acquire(recorded.succeeding());
        assert!(!guard.is_preventing());

        // 0 active downloads: stays false, no change
        assert!(!guard.update(0));
        assert!(!guard.is_preventing());

        // 1 active download: transitions to true
        assert!(guard.update(1));
        assert!(guard.is_preventing());

        // 5 active downloads: stays true, no change — and no second syscall
        assert!(!guard.update(5));
        assert!(guard.is_preventing());
        assert_eq!(recorded.acquired(), 1);

        // 0 active downloads: transitions back to false
        assert!(guard.update(0));
        assert!(!guard.is_preventing());

        // Re-acquire and test release
        assert!(guard.update(2));
        assert!(guard.is_preventing());
        assert!(guard.release());
        assert!(!guard.is_preventing());
        // Releasing twice is a no-op, not a double free.
        assert!(!guard.release());
    }

    #[test]
    fn test_set_enabled_gates_the_lock() {
        let recorded = Recorded::default();
        let guard = PowerGuard::with_acquire(recorded.succeeding());
        assert!(
            !guard.set_enabled(true),
            "the constructor already starts from the default `true`"
        );

        // Armed, but idle: nothing to hold yet.
        assert!(!guard.update(0));
        assert!(!guard.is_preventing());

        // Downloads start while the setting is on: lock taken.
        assert!(guard.update(1));
        assert!(guard.is_preventing());

        // Turning the setting off releases the held lock immediately...
        assert!(guard.set_enabled(false));
        assert!(!guard.is_preventing());
        // ...and further downloads never take it again.
        assert!(!guard.update(3));
        assert!(!guard.is_preventing());
        assert_eq!(recorded.attempts(), 1);

        // Back on while downloads are still running: re-acquired at once, so the
        // toggle is not "until the next task starts".
        assert!(guard.set_enabled(true));
        assert!(guard.is_preventing());

        // Setting the same value again is a no-op.
        assert!(!guard.set_enabled(true));
    }

    #[test]
    fn test_failed_acquire_is_attempted_once_per_edge() {
        let recorded = Recorded::default();
        let guard = PowerGuard::with_acquire(recorded.failing());

        assert!(!guard.update(1), "a failed acquire must not report a lock");
        assert!(!guard.is_preventing());
        // The UI refreshes this constantly; a failing logind call must not turn
        // into one D-Bus round trip (and one warning) per frame.
        assert!(!guard.update(1));
        assert!(!guard.update(7));
        assert_eq!(recorded.attempts(), 1);

        // Falling back to idle re-arms the edge, so the next download retries.
        assert!(!guard.update(0));
        assert!(!guard.update(1));
        assert_eq!(recorded.attempts(), 2);
    }

    /// Eight callers race on one guard. Whatever the interleaving, the lock must
    /// end up held exactly once (never two OS locks, never a lost update).
    #[test]
    fn test_concurrent_updates_hold_one_lock() {
        let recorded = Recorded::default();
        let guard = Arc::new(PowerGuard::with_acquire(recorded.succeeding()));

        let threads: Vec<_> = (0..8)
            .map(|active| {
                let guard = guard.clone();
                std::thread::spawn(move || {
                    for _ in 0..50 {
                        guard.update(active + 1);
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }

        assert!(guard.is_preventing(), "the lock must survive the race");
        assert_eq!(
            recorded.acquired(),
            1,
            "one rising edge, one OS lock"
        );

        assert!(guard.update(0));
        assert!(!guard.is_preventing());
        assert!(!guard.update(0), "every caller already reported idle");
    }
}
