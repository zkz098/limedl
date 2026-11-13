//! Synchronization helpers and Loom model for the Ping-Pong double buffer.
//!
//! Keeps the atomic/mutex primitives for [`super::download_buffer::DownloadBuffer`]
//! cleanly isolated so the flip-token + pre-enabled-notify + background-flush
//! protocol can be exhaustively permutation-tested under `--cfg limedl_loom`.

use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Notify;

/// RAII guard that releases the flip token and notifies waiters on drop.
/// Prevents deadlock if the flip section panics.
pub struct FlipTokenGuard<'a> {
    pub(crate) token: &'a AtomicBool,
    pub(crate) notify: &'a Notify,
}

impl<'a> Drop for FlipTokenGuard<'a> {
    fn drop(&mut self) {
        self.token.store(false, Ordering::SeqCst);
        self.notify.notify_waiters();
    }
}

/// Shared fast-path error check used at the entry and post-flush points of the
/// ping-pong flip loop.
#[inline]
pub(crate) fn check_bg_error(error_flag: &AtomicBool) -> bool {
    error_flag.load(Ordering::Acquire)
}

#[inline]
pub(crate) fn set_bg_error(error_flag: &AtomicBool) {
    error_flag.store(true, Ordering::Release);
}

#[inline]
pub(crate) fn clear_bg_error(error_flag: &AtomicBool) {
    error_flag.store(false, Ordering::Release);
}

#[inline]
pub(crate) fn try_acquire_flip_token(flip_token: &AtomicBool) -> bool {
    flip_token
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
}

#[cfg(all(test, limedl_loom))]
mod loom_tests {
    use loom::sync::{
        Arc as LoomArc, Condvar as LoomCondvar, Mutex as LoomMutex,
        atomic::{
            AtomicBool as LoomAtomicBool, AtomicU64 as LoomAtomicU64, Ordering as LoomOrdering,
        },
    };
    use std::collections::BTreeMap;

    /// Generation-based `Notify` equivalent backed by `loom::sync::{Mutex, Condvar}`
    /// so Loom can explore every wake/wait interleaving of the flip protocol.
    struct LoomFlipNotify {
        state: LoomMutex<u64>,
        condvar: LoomCondvar,
    }

    impl LoomFlipNotify {
        fn new() -> Self {
            Self {
                state: LoomMutex::new(0),
                condvar: LoomCondvar::new(),
            }
        }

        /// Mirrors `tokio::sync::Notify::notified().enable()`: captures the current
        /// generation *before* the caller inspects `flip_token`.
        fn register(&self) -> u64 {
            *self.state.lock().unwrap()
        }

        /// Mirrors `Notified::await`: blocks until `notify_waiters()` advances the
        /// generation past `registered_gen`.
        fn wait(&self, registered_gen: u64) {
            let mut guard = self.state.lock().unwrap();
            while *guard == registered_gen {
                guard = self.condvar.wait(guard).unwrap();
            }
        }

        fn notify_waiters(&self) {
            let mut guard = self.state.lock().unwrap();
            *guard = guard.wrapping_add(1);
            self.condvar.notify_all();
        }
    }

    struct LoomFlipTokenGuard<'a> {
        token: &'a LoomAtomicBool,
        notify: &'a LoomFlipNotify,
    }

    impl<'a> Drop for LoomFlipTokenGuard<'a> {
        fn drop(&mut self) {
            self.token.store(false, LoomOrdering::SeqCst);
            self.notify.notify_waiters();
        }
    }

    /// Minimal state mirror of `PingPongState` using Loom primitives.
    struct LoomHalfState {
        entries: BTreeMap<u64, u64>,
        usage: u64,
    }

    impl LoomHalfState {
        fn new() -> Self {
            Self {
                entries: BTreeMap::new(),
                usage: 0,
            }
        }

        fn try_insert(&mut self, offset: u64, len: u64, half_size: u64) -> bool {
            let prev_len = self.entries.get(&offset).copied().unwrap_or(0);
            let next_usage = self.usage - prev_len + len;
            if next_usage > half_size {
                return false;
            }
            self.entries.insert(offset, len);
            self.usage = next_usage;
            true
        }

        fn drain(&mut self) -> Vec<(u64, u64)> {
            let drained: Vec<(u64, u64)> = std::mem::take(&mut self.entries).into_iter().collect();
            self.usage = 0;
            drained
        }

        fn insert_fresh(&mut self, offset: u64, len: u64) {
            let prev_len = self.entries.insert(offset, len).unwrap_or(0);
            self.usage = self.usage - prev_len + len;
        }
    }

    struct LoomPingPong {
        state: LoomMutex<LoomHalfState>,
        half_size: u64,
        flush_handle: LoomMutex<Option<loom::thread::JoinHandle<()>>>,
        notify: LoomArc<LoomFlipNotify>,
        error_flag: LoomArc<LoomAtomicBool>,
        flip_token: LoomAtomicBool,
        flushed_bytes: LoomArc<LoomAtomicU64>,
        fail_flush: bool,
    }

    impl LoomPingPong {
        fn new(half_size: u64, fail_flush: bool) -> Self {
            Self {
                state: LoomMutex::new(LoomHalfState::new()),
                half_size,
                flush_handle: LoomMutex::new(None),
                notify: LoomArc::new(LoomFlipNotify::new()),
                error_flag: LoomArc::new(LoomAtomicBool::new(false)),
                flip_token: LoomAtomicBool::new(false),
                flushed_bytes: LoomArc::new(LoomAtomicU64::new(0)),
                fail_flush,
            }
        }

        /// Mirrors `DownloadBuffer::buffer_chunk_pingpong_impl` step for step.
        fn buffer_chunk(&self, offset: u64, len: u64) -> Result<(), &'static str> {
            loop {
                if self.error_flag.load(LoomOrdering::Acquire) {
                    return Err("background buffer flush failed");
                }

                if self
                    .state
                    .lock()
                    .unwrap()
                    .try_insert(offset, len, self.half_size)
                {
                    return Ok(());
                }

                // Register notification interest BEFORE attempting to acquire `flip_token`.
                let current_gen = self.notify.register();
                if self
                    .flip_token
                    .compare_exchange(false, true, LoomOrdering::SeqCst, LoomOrdering::SeqCst)
                    .is_err()
                {
                    self.notify.wait(current_gen);
                    continue;
                }

                let _guard = LoomFlipTokenGuard {
                    token: &self.flip_token,
                    notify: &self.notify,
                };

                if self
                    .state
                    .lock()
                    .unwrap()
                    .try_insert(offset, len, self.half_size)
                {
                    return Ok(());
                }

                let prev_handle = self.flush_handle.lock().unwrap().take();
                if let Some(h) = prev_handle {
                    let _ = h.join();
                    if self.error_flag.load(LoomOrdering::Acquire) {
                        return Err("background buffer flush failed");
                    }
                }

                let mut state = self.state.lock().unwrap();
                if state.try_insert(offset, len, self.half_size) {
                    return Ok(());
                }

                let old_entries = state.drain();
                let bg_error = LoomArc::clone(&self.error_flag);
                let bg_notify = LoomArc::clone(&self.notify);
                let bg_flushed = LoomArc::clone(&self.flushed_bytes);
                let fail_flush = self.fail_flush;

                let bg_handle = loom::thread::spawn(move || {
                    if fail_flush {
                        bg_error.store(true, LoomOrdering::Release);
                    } else {
                        let sum: u64 = old_entries.iter().map(|(_, l)| *l).sum();
                        bg_flushed.fetch_add(sum, LoomOrdering::SeqCst);
                    }
                    bg_notify.notify_waiters();
                });

                *self.flush_handle.lock().unwrap() = Some(bg_handle);
                state.insert_fresh(offset, len);
                return Ok(());
            }
        }

        fn flush_all(&self) -> Result<u64, &'static str> {
            let handle = self.flush_handle.lock().unwrap().take();
            if let Some(h) = handle {
                let _ = h.join();
                if self.error_flag.load(LoomOrdering::Acquire) {
                    return Err("background buffer flush failed");
                }
            }
            let entries = self.state.lock().unwrap().drain();
            let sum: u64 = entries.iter().map(|(_, l)| *l).sum();
            self.flushed_bytes.fetch_add(sum, LoomOrdering::SeqCst);
            if self.error_flag.load(LoomOrdering::Acquire) {
                return Err("background buffer flush failed");
            }
            Ok(self.flushed_bytes.load(LoomOrdering::SeqCst))
        }
    }

    #[test]
    fn loom_pingpong_concurrent_writers_no_lost_wakeup_or_bytes() {
        let mut builder = loom::model::Builder::new();
        builder.preemption_bound = Some(2);
        builder.check(|| {
            // Keep total loom threads <= 4 (main + 2 writers + 1 bg_handle):
            // half_size = 10; t1 writes 6+6=12 bytes (1 flip), t2 writes 6 bytes (contends / flips).
            let buf = LoomArc::new(LoomPingPong::new(10, false));

            let b1 = LoomArc::clone(&buf);
            let t1 = loom::thread::spawn(move || {
                b1.buffer_chunk(0, 6).unwrap();
                b1.buffer_chunk(6, 6).unwrap();
            });

            let b2 = LoomArc::clone(&buf);
            let t2 = loom::thread::spawn(move || {
                b2.buffer_chunk(12, 6).unwrap();
            });

            t1.join().unwrap();
            t2.join().unwrap();

            let total = buf.flush_all().unwrap();
            assert_eq!(
                total, 18,
                "all 18 bytes must be durably flushed without loss"
            );
            assert!(!buf.flip_token.load(LoomOrdering::SeqCst));
        });
    }

    #[test]
    fn loom_pingpong_bg_flush_error_propagates_without_hang() {
        let mut builder = loom::model::Builder::new();
        builder.preemption_bound = Some(2);
        builder.check(|| {
            // Keep total loom threads <= 4 (main + 2 writers + 1 bg_handle).
            let buf = LoomArc::new(LoomPingPong::new(10, true));

            let b1 = LoomArc::clone(&buf);
            let t1 = loom::thread::spawn(move || {
                b1.buffer_chunk(0, 6)?;
                b1.buffer_chunk(6, 6)
            });

            let b2 = LoomArc::clone(&buf);
            let t2 = loom::thread::spawn(move || b2.buffer_chunk(12, 6));

            let r1 = t1.join().unwrap();
            let r2 = t2.join().unwrap();

            let flush_res = buf.flush_all();
            let tok = buf.flip_token.load(LoomOrdering::SeqCst);
            let any_err = r1.is_err() || r2.is_err() || flush_res.is_err();
            assert!(
                any_err,
                "background flush error must never be silently lost"
            );
            assert!(
                !tok,
                "flip_token leaked! r1={r1:?}, r2={r2:?}, flush_res={flush_res:?}"
            );
        });
    }
}
