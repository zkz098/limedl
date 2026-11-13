//! Synchronization state machine and Loom model for pending magnet resolution.
//!
//! Coordinates concurrent `pause()` / `resume()` / `cancel()` user actions with
//! the background `session.add_torrent` task so neither side overwrites a newer
//! pause/resume intent or fails with `NotFound` across handoff.

#[cfg(limedl_loom)]
use loom::sync::{Mutex, MutexGuard};
#[cfg(not(limedl_loom))]
use parking_lot::{Mutex, MutexGuard};

/// Outcome reported when recording a user `pause` or `resume` against a
/// pending magnet control block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IntentOutcome {
    /// Background resolution still owns handoff and will reconcile the
    /// torrent session state to the newly recorded intent before finishing.
    PendingHandled,
    /// Background resolution already completed handoff; the caller must apply
    /// the action directly on the live session handle.
    HandedOff,
    /// The pending magnet was canceled/removed concurrently.
    Canceled,
}

/// Next step required by the background resolution task once `add_torrent`
/// has placed the torrent handle into `session`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HandoffStep {
    /// Enforce `target_paused` on the session handle, then poll again in case
    /// another `pause`/`resume` arrived while the async call was in flight.
    ApplyPaused(bool),
    /// Session state matches latest user intent and handoff is closed; caller
    /// may remove the entry from `pending_magnets`.
    HandedOff,
    /// Canceled concurrently; caller should delete any created handle and exit.
    Canceled,
}

#[derive(Debug, Clone, Copy)]
struct PendingState {
    desired_paused: bool,
    handed_off: bool,
    canceled: bool,
}

/// Per-magnet coordination block shared between `PendingMagnet` and its
/// background resolver task.
#[derive(Debug)]
pub struct PendingMagnetControl {
    inner: Mutex<PendingState>,
}

impl PendingMagnetControl {
    pub(crate) fn new(initial_paused: bool) -> Self {
        Self {
            inner: Mutex::new(PendingState {
                desired_paused: initial_paused,
                handed_off: false,
                canceled: false,
            }),
        }
    }

    #[cfg(not(limedl_loom))]
    fn lock_state(&self) -> MutexGuard<'_, PendingState> {
        self.inner.lock()
    }

    #[cfg(limedl_loom)]
    fn lock_state(&self) -> MutexGuard<'_, PendingState> {
        self.inner.lock().expect("pending magnet mutex poisoned")
    }

    pub(crate) fn is_paused(&self) -> bool {
        self.lock_state().desired_paused
    }

    pub(crate) fn is_canceled(&self) -> bool {
        self.lock_state().canceled
    }

    pub(crate) fn mark_canceled(&self) {
        self.lock_state().canceled = true;
    }

    /// Record a user `pause` (`true`) or `resume` (`false`) request.
    pub(crate) fn record_user_intent(&self, paused: bool) -> IntentOutcome {
        let mut st = self.lock_state();
        if st.canceled {
            return IntentOutcome::Canceled;
        }
        st.desired_paused = paused;
        if st.handed_off {
            IntentOutcome::HandedOff
        } else {
            IntentOutcome::PendingHandled
        }
    }

    /// Advance the post-`add_torrent` handoff loop given the session state
    /// last known to have been applied by the background task.
    pub(crate) fn poll_handoff(&self, applied_paused: bool) -> HandoffStep {
        let mut st = self.lock_state();
        if st.canceled {
            return HandoffStep::Canceled;
        }
        if st.desired_paused != applied_paused {
            return HandoffStep::ApplyPaused(st.desired_paused);
        }
        st.handed_off = true;
        HandoffStep::HandedOff
    }
}

#[cfg(all(test, limedl_loom))]
mod loom_tests {
    use super::*;
    use loom::sync::atomic::{AtomicBool, Ordering};
    use loom::sync::{Arc, Mutex as LoomMutex};
    use std::collections::HashMap;

    struct SimSession {
        torrents: LoomMutex<HashMap<u64, bool>>,
        pending: LoomMutex<HashMap<u64, Arc<PendingMagnetControl>>>,
        final_user_paused: AtomicBool,
    }

    impl SimSession {
        fn new(initial_paused: bool) -> (Self, Arc<PendingMagnetControl>) {
            let control = Arc::new(PendingMagnetControl::new(initial_paused));
            let mut pending = HashMap::new();
            pending.insert(1, Arc::clone(&control));
            (
                Self {
                    torrents: LoomMutex::new(HashMap::new()),
                    pending: LoomMutex::new(pending),
                    final_user_paused: AtomicBool::new(initial_paused),
                },
                control,
            )
        }

        fn bg_resolve(&self, id: u64, control: &PendingMagnetControl, initial_paused: bool) {
            if control.is_canceled() {
                return;
            }
            // Step 1: session.add_torrent inserts handle with initial_paused.
            {
                let mut t = self.torrents.lock().expect("torrents poisoned");
                t.insert(id, initial_paused);
            }

            // Step 2: reconcile user pause/resume intents before removing pending entry.
            let mut applied_paused = initial_paused;
            loop {
                match control.poll_handoff(applied_paused) {
                    HandoffStep::Canceled => {
                        self.torrents.lock().expect("torrents poisoned").remove(&id);
                        self.pending.lock().expect("pending poisoned").remove(&id);
                        return;
                    }
                    HandoffStep::ApplyPaused(target) => {
                        loom::thread::yield_now();
                        let mut t = self.torrents.lock().expect("torrents poisoned");
                        if let Some(state) = t.get_mut(&id) {
                            *state = target;
                        }
                        applied_paused = target;
                    }
                    HandoffStep::HandedOff => {
                        self.pending.lock().expect("pending poisoned").remove(&id);
                        return;
                    }
                }
            }
        }

        fn user_set_paused(&self, id: u64, paused: bool) -> Result<(), &'static str> {
            let control = self
                .pending
                .lock()
                .expect("pending poisoned")
                .get(&id)
                .cloned();
            if let Some(control) = control {
                match control.record_user_intent(paused) {
                    IntentOutcome::PendingHandled => {
                        self.final_user_paused.store(paused, Ordering::Release);
                        return Ok(());
                    }
                    IntentOutcome::Canceled => return Err("not found"),
                    IntentOutcome::HandedOff => {}
                }
            }
            let mut t = self.torrents.lock().expect("torrents poisoned");
            let entry = t.get_mut(&id).ok_or("not found")?;
            *entry = paused;
            self.final_user_paused.store(paused, Ordering::Release);
            Ok(())
        }
    }

    #[test]
    fn bg_resolve_vs_resume_from_start_paused_never_repauses() {
        loom::model(|| {
            let (sim, control) = SimSession::new(true);
            let sim = Arc::new(sim);

            let t_bg = {
                let sim = Arc::clone(&sim);
                let control = Arc::clone(&control);
                loom::thread::spawn(move || {
                    sim.bg_resolve(1, &control, true);
                })
            };

            let t_resume = {
                let sim = Arc::clone(&sim);
                loom::thread::spawn(move || {
                    sim.user_set_paused(1, false)
                        .expect("resume during magnet resolution must never return NotFound");
                })
            };

            t_bg.join().unwrap();
            t_resume.join().unwrap();

            assert!(sim.pending.lock().expect("pending poisoned").is_empty());
            let torrents = sim.torrents.lock().expect("torrents poisoned");
            assert_eq!(
                torrents.get(&1),
                Some(&false),
                "start_paused magnet was silently re-paused after user resume"
            );
        });
    }

    #[test]
    fn bg_resolve_vs_pause_then_resume_converges_without_not_found() {
        loom::model(|| {
            let (sim, control) = SimSession::new(false);
            let sim = Arc::new(sim);

            let t_bg = {
                let sim = Arc::clone(&sim);
                let control = Arc::clone(&control);
                loom::thread::spawn(move || {
                    sim.bg_resolve(1, &control, false);
                })
            };

            let t_user = {
                let sim = Arc::clone(&sim);
                loom::thread::spawn(move || {
                    sim.user_set_paused(1, true)
                        .expect("pause during handoff must not fail NotFound");
                    sim.user_set_paused(1, false)
                        .expect("resume during handoff must not fail NotFound");
                })
            };

            t_bg.join().unwrap();
            t_user.join().unwrap();

            assert!(sim.pending.lock().expect("pending poisoned").is_empty());
            let torrents = sim.torrents.lock().expect("torrents poisoned");
            assert_eq!(torrents.get(&1), Some(&false));
        });
    }
}
