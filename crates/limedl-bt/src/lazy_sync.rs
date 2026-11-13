//! Synchronization helpers and Loom verification for [`super::lazy::LazyBtBackend`].
//!
//! Keeps the idle-settle window predicate and lifecycle/shutdown/idle-unload
//! state-machine model isolated so [`super::lazy`] stays focused and
//! exhaustive permutation tests run cleanly under `--cfg limedl_loom`.

/// Whether at least `settle_ms` have elapsed since `last_activity_ms`.
#[inline]
pub(crate) fn idle_settle_elapsed(last_activity_ms: u64, now_ms: u64, settle_ms: u64) -> bool {
    now_ms.saturating_sub(last_activity_ms) >= settle_ms
}

/// Remove resume data left by the previous BitTorrent engine and return how
/// many files were deleted.
///
/// irontide wrote `<state>/resume/torrents/<hex>.resume`; some older builds also
/// left `.resume` files directly under `<state>/resume`. librqbit's JSON
/// persistence only writes `session.json`, `<hash>.torrent` and `<hash>.bitv`
/// there, so no live state uses the `.resume` extension.
pub fn take_legacy_resume_files(state_dir: &std::path::Path) -> usize {
    let resume_dir = state_dir.join("resume");
    let removed =
        remove_resume_files(&resume_dir) + remove_resume_files(&resume_dir.join("torrents"));
    if removed > 0 {
        tracing::warn!(
            removed,
            "removed resume files left by the previous BitTorrent engine; those tasks cannot be restored"
        );
    }
    removed
}

fn remove_resume_files(dir: &std::path::Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| {
            entry.path().extension().and_then(std::ffi::OsStr::to_str) == Some("resume")
        })
        .filter(|entry| std::fs::remove_file(entry.path()).is_ok())
        .count()
}

#[cfg(all(test, limedl_loom))]
mod loom_tests {
    use super::idle_settle_elapsed;
    use loom::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
    use loom::sync::{Arc, Mutex, RwLock};

    const SETTLE_MS: u64 = 20_000;

    /// Minimal synchronous twin of `LazyBtBackend`'s engine slot, activity
    /// stamp, shutdown flag, and lifecycle serializer.
    struct SimLazy {
        engine: RwLock<Option<usize>>,
        lifecycle: Mutex<()>,
        shutting_down: AtomicBool,
        last_activity_ms: AtomicU64,
        has_active_torrents: AtomicBool,
        next_engine_id: AtomicUsize,
        alive_engines: AtomicUsize,
        engine_shut_down: Mutex<Vec<bool>>,
    }

    impl SimLazy {
        fn new_ready(initial_activity_ms: u64) -> Self {
            Self {
                engine: RwLock::new(Some(0)),
                lifecycle: Mutex::new(()),
                shutting_down: AtomicBool::new(false),
                last_activity_ms: AtomicU64::new(initial_activity_ms),
                has_active_torrents: AtomicBool::new(false),
                next_engine_id: AtomicUsize::new(1),
                alive_engines: AtomicUsize::new(1),
                engine_shut_down: Mutex::new(vec![false]),
            }
        }

        fn new_unloaded() -> Self {
            Self {
                engine: RwLock::new(None),
                lifecycle: Mutex::new(()),
                shutting_down: AtomicBool::new(false),
                last_activity_ms: AtomicU64::new(0),
                has_active_torrents: AtomicBool::new(false),
                next_engine_id: AtomicUsize::new(0),
                alive_engines: AtomicUsize::new(0),
                engine_shut_down: Mutex::new(Vec::new()),
            }
        }

        fn engine_if_ready(&self) -> Option<usize> {
            if self.shutting_down.load(Ordering::Acquire) {
                return None;
            }
            *self.engine.read().expect("rwlock poisoned")
        }

        fn create_engine(&self) -> usize {
            let id = self.next_engine_id.fetch_add(1, Ordering::AcqRel);
            self.alive_engines.fetch_add(1, Ordering::AcqRel);
            let mut flags = self.engine_shut_down.lock().expect("mutex poisoned");
            if flags.len() <= id {
                flags.resize(id + 1, false);
            }
            flags[id] = false;
            id
        }

        fn shut_engine_down(&self, id: usize) {
            let mut flags = self.engine_shut_down.lock().expect("mutex poisoned");
            if !flags[id] {
                flags[id] = true;
                self.alive_engines.fetch_sub(1, Ordering::AcqRel);
            }
        }

        fn acquire_engine(&self, call_now_ms: u64) -> Option<usize> {
            if self.shutting_down.load(Ordering::Acquire) {
                return None;
            }
            self.last_activity_ms.store(call_now_ms, Ordering::Release);
            if let Some(id) = self.engine_if_ready() {
                return Some(id);
            }
            let _guard = self.lifecycle.lock().expect("lifecycle poisoned");
            if let Some(id) = self.engine_if_ready() {
                return Some(id);
            }
            if self.shutting_down.load(Ordering::Acquire) {
                return None;
            }
            let id = self.create_engine();
            if self.shutting_down.load(Ordering::Acquire) {
                self.shut_engine_down(id);
                return None;
            }
            *self.engine.write().expect("rwlock poisoned") = Some(id);
            self.last_activity_ms.store(call_now_ms, Ordering::Release);
            Some(id)
        }

        fn stop_idle_engine(&self, check_now_ms: u64) {
            let _guard = self.lifecycle.lock().expect("lifecycle poisoned");
            if self.engine.read().expect("rwlock poisoned").is_none() {
                return;
            }
            if self.has_active_torrents.load(Ordering::Acquire) {
                return;
            }
            let engine = {
                let mut slot = self.engine.write().expect("rwlock poisoned");
                if !idle_settle_elapsed(
                    self.last_activity_ms.load(Ordering::Acquire),
                    check_now_ms,
                    SETTLE_MS,
                ) {
                    return;
                }
                slot.take()
            };
            if let Some(id) = engine {
                self.shut_engine_down(id);
            }
        }

        fn shutdown(&self) {
            self.shutting_down.store(true, Ordering::Release);
            let _guard = self.lifecycle.lock().expect("lifecycle poisoned");
            if let Some(id) = self.engine.write().expect("rwlock poisoned").take() {
                self.shut_engine_down(id);
            }
        }
    }

    #[test]
    fn engine_fast_path_vs_stop_idle_never_unloads_active_caller() {
        loom::model(|| {
            // Engine 0 was last active at t=0; at t=25_000 the idle supervisor
            // concluded settle elapsed and is about to invoke stop_idle_engine().
            let lazy = Arc::new(SimLazy::new_ready(0));

            let caller = {
                let lazy = Arc::clone(&lazy);
                loom::thread::spawn(move || {
                    let id = lazy
                        .acquire_engine(26_000)
                        .expect("engine must start or stay available");
                    // Yield point before torrent is inserted into session.
                    loom::thread::yield_now();
                    lazy.has_active_torrents.store(true, Ordering::Release);
                    let flags = lazy.engine_shut_down.lock().expect("mutex poisoned");
                    assert!(
                        !flags[id],
                        "engine {id} handed to start() caller was torn down before torrent insertion!"
                    );
                })
            };

            let idle_stopper = {
                let lazy = Arc::clone(&lazy);
                loom::thread::spawn(move || {
                    lazy.stop_idle_engine(26_500);
                })
            };

            caller.join().unwrap();
            idle_stopper.join().unwrap();
            assert_eq!(lazy.alive_engines.load(Ordering::Acquire), 1);
        });
    }

    #[test]
    fn concurrent_engine_create_vs_shutdown_never_leaks_live_engine() {
        loom::model(|| {
            let lazy = Arc::new(SimLazy::new_unloaded());

            let t_start = {
                let lazy = Arc::clone(&lazy);
                loom::thread::spawn(move || {
                    let _ = lazy.acquire_engine(1_000);
                })
            };

            let t_shutdown = {
                let lazy = Arc::clone(&lazy);
                loom::thread::spawn(move || {
                    lazy.shutdown();
                })
            };

            t_start.join().unwrap();
            t_shutdown.join().unwrap();

            assert!(lazy.shutting_down.load(Ordering::Acquire));
            assert!(lazy.engine.read().expect("rwlock poisoned").is_none());
            assert_eq!(
                lazy.alive_engines.load(Ordering::Acquire),
                0,
                "concurrent create vs shutdown leaked a running engine"
            );
        });
    }
}
