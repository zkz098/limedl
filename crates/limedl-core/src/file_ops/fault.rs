//! Deterministic write-fault injection for the I/O worker batch-flush path.
//!
//! Lets tests force a specific background `write_batch` to fail for a particular
//! file, reproducing the real-world "SSD/HDD buffer flush failed" error path
//! (background `error_flag` → degraded → direct-write fallback → `flush_all`
//! surfaces the error). Targeting is by file pointer so that in a parallel test
//! run this never disturbs a different download writing to a different file.
//!
//! Inert unless armed: production code never calls `arm`, and the whole module
//! is `cfg`-gated so it compiles out unless tests / `test-utils` are enabled.

#[cfg(any(test, feature = "test-utils"))]
use std::collections::HashMap;
#[cfg(any(test, feature = "test-utils"))]
use std::fs::File;
#[cfg(any(test, feature = "test-utils"))]
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
#[cfg(any(test, feature = "test-utils"))]
use std::sync::{Arc, Mutex, OnceLock};

#[cfg(any(test, feature = "test-utils"))]
static TARGET: AtomicUsize = AtomicUsize::new(0);
#[cfg(any(test, feature = "test-utils"))]
static COUNTDOWN: AtomicI64 = AtomicI64::new(-1);

/// Registry of currently-created download temp files, keyed by download id.
/// Lets a pipeline test resolve the pointer of *its own* download without
/// racing concurrent downloads in other tests.
#[cfg(any(test, feature = "test-utils"))]
fn registry() -> &'static Mutex<HashMap<String, usize>> {
    static R: OnceLock<Mutex<HashMap<String, usize>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Register a download's temp file pointer, keyed by download id.
#[cfg(any(test, feature = "test-utils"))]
pub fn register_file(id: &str, file: &Arc<File>) {
    registry()
        .lock()
        .unwrap()
        .insert(id.to_string(), Arc::as_ptr(file) as usize);
}

/// Resolve a download id to its registered temp-file pointer, if any.
#[cfg(any(test, feature = "test-utils"))]
pub fn file_ptr_for(id: &str) -> Option<usize> {
    registry().lock().unwrap().get(id).copied()
}

/// Arm a one-shot fault for `file_ptr`: the `fail_index`-th `write_batch`
/// from now (`0` = the very next batch) will fail with an injected I/O error.
///
/// Returns a guard that resets all fault state on drop, so a panicking test
/// cannot leak an armed fault into other tests.
#[cfg(any(test, feature = "test-utils"))]
pub fn arm(file_ptr: usize, fail_index: i64) -> FaultGuard {
    TARGET.store(file_ptr, Ordering::SeqCst);
    COUNTDOWN.store(fail_index, Ordering::SeqCst);
    FaultGuard
}

/// Consume one batch-write allowance for `file_ptr`. Returns `true` when
/// this batch must fail. Only the targeted file is counted; any other file
/// (another download in a parallel test) passes through untouched.
#[cfg(any(test, feature = "test-utils"))]
pub fn consume_batch(file_ptr: usize) -> bool {
    if TARGET.load(Ordering::SeqCst) != file_ptr {
        return false;
    }
    loop {
        let cur = COUNTDOWN.load(Ordering::SeqCst);
        if cur < 0 {
            return false;
        }
        let next = cur - 1;
        if COUNTDOWN
            .compare_exchange_weak(cur, next, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            if cur == 0 {
                reset();
                return true;
            }
            return false;
        }
    }
}

/// Clear all fault state (disarm).
#[cfg(any(test, feature = "test-utils"))]
pub fn reset() {
    TARGET.store(0, Ordering::SeqCst);
    COUNTDOWN.store(-1, Ordering::SeqCst);
}

/// RAII guard: clears fault state on drop.
#[cfg(any(test, feature = "test-utils"))]
pub struct FaultGuard;
#[cfg(any(test, feature = "test-utils"))]
impl Drop for FaultGuard {
    fn drop(&mut self) {
        reset();
    }
}

/// Global lock serialising all fault-injecting tests.
///
/// The fault state is a single set of globals, so two tests arming at once
/// would clobber each other's target. Tests that call [`arm`] must acquire
/// this lock for their whole body so only one runs at a time. Uses a tokio
/// async mutex so it is safe to hold across `.await` in `#[tokio::test]`.
#[cfg(any(test, feature = "test-utils"))]
pub async fn injection_lock() -> tokio::sync::OwnedMutexGuard<()> {
    static LOCK: OnceLock<Arc<tokio::sync::Mutex<()>>> = OnceLock::new();
    LOCK.get_or_init(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone()
        .lock_owned()
        .await
}
