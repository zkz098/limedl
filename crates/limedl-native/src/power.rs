use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(windows)]
mod imp {
    use windows::Win32::System::Power::{
        SetThreadExecutionState, ES_AWAYMODE_REQUIRED, ES_CONTINUOUS, ES_SYSTEM_REQUIRED,
    };

    pub fn prevent_sleep() {
        unsafe {
            // ES_CONTINUOUS ensures the state remains in effect until the next call.
            // ES_SYSTEM_REQUIRED keeps the system (CPU and network) awake.
            // ES_AWAYMODE_REQUIRED allows the display to turn off while background download continues.
            SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_AWAYMODE_REQUIRED);
        }
        tracing::info!("系统电源管理: 已启用下载防休眠锁 (Keep-Awake)");
    }

    pub fn allow_sleep() {
        unsafe {
            // Revert back to continuous normal power management
            SetThreadExecutionState(ES_CONTINUOUS);
        }
        tracing::info!("系统电源管理: 已释放下载防休眠锁 (Restore Normal Sleep)");
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn prevent_sleep() {}
    pub fn allow_sleep() {}
}

/// Power guard that manages system sleep state based on active downloads.
#[derive(Debug, Default)]
pub struct PowerGuard {
    is_preventing: AtomicBool,
}

impl PowerGuard {
    pub fn new() -> Self {
        Self {
            is_preventing: AtomicBool::new(false),
        }
    }

    /// Update sleep prevention based on active download count.
    /// Returns `true` if the state changed.
    pub fn update(&self, active_downloads: usize) -> bool {
        if active_downloads > 0 {
            if !self.is_preventing.swap(true, Ordering::SeqCst) {
                imp::prevent_sleep();
                return true;
            }
        } else if self.is_preventing.swap(false, Ordering::SeqCst) {
            imp::allow_sleep();
            return true;
        }
        false
    }

    /// Explicitly release sleep prevention (e.g. on application exit).
    pub fn release(&self) {
        if self.is_preventing.swap(false, Ordering::SeqCst) {
            imp::allow_sleep();
        }
    }

    /// Check if sleep prevention is currently active.
    #[allow(dead_code)]
    pub fn is_preventing(&self) -> bool {
        self.is_preventing.load(Ordering::SeqCst)
    }
}

impl Drop for PowerGuard {
    fn drop(&mut self) {
        self.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_guard_transitions() {
        let guard = PowerGuard::new();
        assert!(!guard.is_preventing());

        // 0 active downloads: stays false, no change
        assert!(!guard.update(0));
        assert!(!guard.is_preventing());

        // 1 active download: transitions to true
        assert!(guard.update(1));
        assert!(guard.is_preventing());

        // 5 active downloads: stays true, no change
        assert!(!guard.update(5));
        assert!(guard.is_preventing());

        // 0 active downloads: transitions back to false
        assert!(guard.update(0));
        assert!(!guard.is_preventing());

        // Re-acquire and test release
        assert!(guard.update(2));
        assert!(guard.is_preventing());
        guard.release();
        assert!(!guard.is_preventing());
    }
}
