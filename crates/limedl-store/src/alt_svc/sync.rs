//! Ordered single-writer persistence actor for `alt_svc.redb`.
//!
//! All L2 (`alt_svc.redb`) mutations are serialized onto a dedicated OS thread
//! via an unbounded MPSC channel. Callers enqueue commands **while holding** the
//! L1 `DashMap` shard write lock for the affected origin, guaranteeing that L2
//! commit order strictly matches L1 mutation order.

use std::sync::{Arc, mpsc};
use std::thread::JoinHandle;

use parking_lot::Mutex;
use tracing::warn;

use super::AltSvcRecord;
use crate::database::Database;

pub(crate) enum AltSvcWriteCmd {
    Upsert(Vec<AltSvcRecord>),
    ClearOrigin { host: String, port: u16 },
    CleanExpired { now_ms: u64 },
    Shutdown(mpsc::SyncSender<()>),
}

pub(crate) struct AltSvcWriter {
    tx: mpsc::Sender<AltSvcWriteCmd>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl AltSvcWriter {
    pub(crate) fn spawn(db: Arc<Database>) -> Self {
        let (tx, rx) = mpsc::channel::<AltSvcWriteCmd>();
        let thread = std::thread::Builder::new()
            .name("limedl-altsvc-writer".to_string())
            .spawn(move || run_altsvc_writer(&db, rx))
            .ok();
        Self {
            tx,
            thread: Mutex::new(thread),
        }
    }

    pub(crate) fn send(&self, cmd: AltSvcWriteCmd) {
        let _ = self.tx.send(cmd);
    }

    pub(crate) fn shutdown(&self) {
        let Some(handle) = self.thread.lock().take() else {
            return;
        };
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        if self.tx.send(AltSvcWriteCmd::Shutdown(reply_tx)).is_ok() {
            let _ = reply_rx.recv();
        }
        let _ = handle.join();
    }
}

impl Drop for AltSvcWriter {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn run_altsvc_writer(db: &Database, rx: mpsc::Receiver<AltSvcWriteCmd>) {
    while let Ok(cmd) = rx.recv() {
        match cmd {
            AltSvcWriteCmd::Upsert(records) => {
                for rec in records {
                    if let Err(e) = db.upsert_alt_svc(&rec) {
                        warn!(
                            "Failed to persist Alt-Svc {}:{} -> {}:{}: {e}",
                            rec.origin_host, rec.origin_port, rec.alt_host, rec.alt_port
                        );
                    }
                }
            }
            AltSvcWriteCmd::ClearOrigin { host, port } => {
                if let Err(e) = db.clear_alt_svc_for_origin(&host, port) {
                    warn!("Failed to clear Alt-Svc in db for {host}:{port}: {e}");
                }
            }
            AltSvcWriteCmd::CleanExpired { now_ms } => {
                if let Err(e) = db.clean_expired_alt_svc(now_ms) {
                    warn!("Failed to clean expired Alt-Svc entries: {e}");
                }
            }
            AltSvcWriteCmd::Shutdown(ack) => {
                let _ = ack.send(());
                break;
            }
        }
    }
}

#[cfg(all(test, limedl_loom))]
mod loom_tests {
    use loom::sync::{Arc, Mutex};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum State {
        Empty,
        Valid { broken: bool },
    }

    #[derive(Clone, Copy, Debug)]
    enum Cmd {
        Set(State),
        Clear,
    }

    /// Exhaustively verify under Loom that enqueuing L2 commands *while holding*
    /// the L1 shard lock guarantees L2's replayed final state always equals L1's
    /// final state across all interleavings of store, mark_broken, and clear.
    #[test]
    fn loom_l1_and_l2_converge_when_enqueued_under_shard_lock() {
        loom::model(|| {
            let l1 = Arc::new(Mutex::new(State::Empty));
            let l2_log = Arc::new(Mutex::new(Vec::<Cmd>::new()));

            let l1_a = Arc::clone(&l1);
            let log_a = Arc::clone(&l2_log);
            let t1 = loom::thread::spawn(move || {
                let mut g = l1_a.lock().unwrap();
                *g = State::Valid { broken: false };
                log_a.lock().unwrap().push(Cmd::Set(*g));
            });

            let l1_b = Arc::clone(&l1);
            let log_b = Arc::clone(&l2_log);
            let t2 = loom::thread::spawn(move || {
                let mut g = l1_b.lock().unwrap();
                if let State::Valid { .. } = *g {
                    *g = State::Valid { broken: true };
                    log_b.lock().unwrap().push(Cmd::Set(*g));
                }
            });

            let l1_c = Arc::clone(&l1);
            let log_c = Arc::clone(&l2_log);
            let t3 = loom::thread::spawn(move || {
                let mut g = l1_c.lock().unwrap();
                *g = State::Empty;
                log_c.lock().unwrap().push(Cmd::Clear);
            });

            t1.join().unwrap();
            t2.join().unwrap();
            t3.join().unwrap();

            let final_l1 = *l1.lock().unwrap();
            let mut replayed_l2 = State::Empty;
            for cmd in l2_log.lock().unwrap().iter().copied() {
                replayed_l2 = match cmd {
                    Cmd::Set(s) => s,
                    Cmd::Clear => State::Empty,
                };
            }
            assert_eq!(replayed_l2, final_l1);
        });
    }
}
