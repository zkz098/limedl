//! Single-writer persistence actor for `core_db` (`downloads.redb`).
//!
//! All mutating operations on `ACTIVE_DOWNLOADS`, `TERMINAL_DOWNLOADS`, and
//! `CHUNK_PROGRESS` are serialized onto one dedicated OS writer thread via an
//! unbounded MPSC channel. On each wakeup the actor drains all queued commands,
//! coalesces multiple progress snapshots targeting the same `download_id`, and
//! commits them in a single `redb` write transaction.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, mpsc};
use std::thread::JoinHandle;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use redb::Durability;
use tokio::sync::oneshot;

use super::chunk_repo::{ProgressBatchEntry, apply_progress_entry};
#[cfg(test)]
use super::manifest_repo::apply_update_download;
use super::manifest_repo::{apply_delete_download, apply_insert_download, apply_set_priority};
use super::schema::{ACTIVE_DOWNLOADS, CHUNK_PROGRESS};
use crate::manifest::{ChunkManifest, Manifest};

pub(crate) enum ReplyChannel {
    Sync(mpsc::SyncSender<Result<()>>),
    Async(oneshot::Sender<Result<()>>),
}

impl ReplyChannel {
    pub(crate) fn send(self, res: Result<()>) {
        match self {
            Self::Sync(tx) => {
                let _ = tx.send(res);
            }
            Self::Async(tx) => {
                let _ = tx.send(res);
            }
        }
    }
}

pub(crate) fn sync_reply() -> (ReplyChannel, mpsc::Receiver<Result<()>>) {
    let (tx, rx) = mpsc::sync_channel(1);
    (ReplyChannel::Sync(tx), rx)
}

pub(crate) fn async_reply() -> (ReplyChannel, oneshot::Receiver<Result<()>>) {
    let (tx, rx) = oneshot::channel();
    (ReplyChannel::Async(tx), rx)
}

pub(crate) enum WriteCommand {
    InsertDownload {
        manifest: Box<Manifest>,
        reply: ReplyChannel,
    },
    #[cfg(test)]
    UpdateDownload {
        manifest: Box<Manifest>,
        reply: ReplyChannel,
    },
    DeleteDownload {
        id: String,
        reply: ReplyChannel,
    },
    SetPriority {
        id: String,
        priority: u8,
        reply: ReplyChannel,
    },
    Progress {
        entry: ProgressBatchEntry,
        reply: Option<ReplyChannel>,
    },
    ProgressBatch {
        entries: Vec<ProgressBatchEntry>,
        reply: Option<ReplyChannel>,
    },
    Shutdown {
        reply: ReplyChannel,
    },
}

pub(crate) struct WriteActorHandle {
    tx: mpsc::Sender<WriteCommand>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl WriteActorHandle {
    pub(crate) fn spawn(core_db: Arc<redb::Database>) -> Result<Self> {
        let (tx, rx) = mpsc::channel::<WriteCommand>();
        let thread = std::thread::Builder::new()
            .name("limedl-db-writer".to_string())
            .spawn(move || run_writer_loop(&core_db, rx))
            .context("failed to spawn database writer thread")?;
        Ok(Self {
            tx,
            thread: Mutex::new(Some(thread)),
        })
    }

    pub(crate) fn send(&self, cmd: WriteCommand) -> Result<()> {
        self.tx
            .send(cmd)
            .map_err(|_| anyhow::anyhow!("database writer actor has stopped"))
    }

    pub(crate) fn shutdown(&self) -> Result<()> {
        let mut guard = self.thread.lock();
        let Some(handle) = guard.take() else {
            return Ok(());
        };
        let (reply_tx, reply_rx) = sync_reply();
        if self
            .tx
            .send(WriteCommand::Shutdown { reply: reply_tx })
            .is_ok()
        {
            let _ = reply_rx.recv();
        }
        drop(guard);
        let _ = handle.join();
        Ok(())
    }
}

impl Drop for WriteActorHandle {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn merge_progress_entry(existing: &mut ProgressBatchEntry, incoming: ProgressBatchEntry) {
    existing.downloaded_bytes = incoming.downloaded_bytes;
    existing.state = incoming.state;
    existing.updated_at_ms = incoming.updated_at_ms;
    if !incoming.dirty_chunks.is_empty() {
        let mut by_idx: BTreeMap<usize, ChunkManifest> = existing
            .dirty_chunks
            .drain(..)
            .map(|c| (c.index, c))
            .collect();
        for chunk in incoming.dirty_chunks {
            by_idx.insert(chunk.index, chunk);
        }
        existing.dirty_chunks = by_idx.into_values().collect();
    }
}

fn coalesce_progress(pending: &mut HashMap<String, ProgressBatchEntry>, entry: ProgressBatchEntry) {
    match pending.get_mut(&entry.id) {
        Some(existing) => merge_progress_entry(existing, entry),
        None => {
            pending.insert(entry.id.clone(), entry);
        }
    }
}

fn flush_coalesced_progress(
    core_db: &redb::Database,
    pending: &mut HashMap<String, ProgressBatchEntry>,
    replies: &mut Vec<ReplyChannel>,
) {
    if pending.is_empty() {
        for reply in replies.drain(..) {
            reply.send(Ok(()));
        }
        return;
    }

    let result = (|| -> Result<()> {
        let mut tx = core_db
            .begin_write()
            .context("failed to begin progress write transaction")?;
        tx.set_durability(Durability::None)?;
        {
            let mut active = tx.open_table(ACTIVE_DOWNLOADS)?;
            let mut chunks = tx.open_table(CHUNK_PROGRESS)?;
            for entry in pending.values() {
                apply_progress_entry(
                    &mut active,
                    &mut chunks,
                    &entry.id,
                    entry.downloaded_bytes,
                    &entry.dirty_chunks,
                    entry.state,
                    entry.updated_at_ms,
                )?;
            }
        }
        tx.commit()
            .context("failed to commit coalesced progress transaction")?;
        Ok(())
    })();

    pending.clear();
    if let Err(ref err) = result {
        tracing::error!("database writer progress flush failed: {err:#}");
    }

    let mut iter = replies.drain(..);
    if let Some(first) = iter.next() {
        match result {
            Ok(()) => {
                first.send(Ok(()));
                for reply in iter {
                    reply.send(Ok(()));
                }
            }
            Err(err) => {
                let msg = format!("{err:#}");
                first.send(Err(err));
                for reply in iter {
                    reply.send(Err(anyhow::anyhow!("{msg}")));
                }
            }
        }
    }
}

fn flush_durable_barrier(core_db: &redb::Database) -> Result<()> {
    let mut tx = core_db
        .begin_write()
        .context("failed to begin shutdown flush transaction")?;
    tx.set_durability(Durability::Immediate)?;
    tx.commit()
        .context("failed to flush database on shutdown")?;
    Ok(())
}

fn run_writer_loop(core_db: &redb::Database, rx: mpsc::Receiver<WriteCommand>) {
    let mut pending_progress: HashMap<String, ProgressBatchEntry> = HashMap::new();
    let mut progress_replies: Vec<ReplyChannel> = Vec::new();

    while let Ok(first) = rx.recv() {
        let mut batch = vec![first];
        while let Ok(cmd) = rx.try_recv() {
            batch.push(cmd);
        }

        for cmd in batch {
            match cmd {
                WriteCommand::Progress { entry, reply } => {
                    coalesce_progress(&mut pending_progress, entry);
                    if let Some(r) = reply {
                        progress_replies.push(r);
                    }
                }
                WriteCommand::ProgressBatch { entries, reply } => {
                    for entry in entries {
                        coalesce_progress(&mut pending_progress, entry);
                    }
                    if let Some(r) = reply {
                        progress_replies.push(r);
                    }
                }
                WriteCommand::InsertDownload { manifest, reply } => {
                    pending_progress.remove(&manifest.id);
                    flush_coalesced_progress(core_db, &mut pending_progress, &mut progress_replies);
                    let res = apply_insert_download(core_db, &manifest);
                    reply.send(res);
                }
                #[cfg(test)]
                WriteCommand::UpdateDownload { manifest, reply } => {
                    pending_progress.remove(&manifest.id);
                    flush_coalesced_progress(core_db, &mut pending_progress, &mut progress_replies);
                    let res = apply_update_download(core_db, &manifest);
                    reply.send(res);
                }
                WriteCommand::DeleteDownload { id, reply } => {
                    pending_progress.remove(&id);
                    flush_coalesced_progress(core_db, &mut pending_progress, &mut progress_replies);
                    let res = apply_delete_download(core_db, &id);
                    reply.send(res);
                }
                WriteCommand::SetPriority {
                    id,
                    priority,
                    reply,
                } => {
                    flush_coalesced_progress(core_db, &mut pending_progress, &mut progress_replies);
                    let res = apply_set_priority(core_db, &id, priority);
                    reply.send(res);
                }
                WriteCommand::Shutdown { reply } => {
                    flush_coalesced_progress(core_db, &mut pending_progress, &mut progress_replies);
                    let res = flush_durable_barrier(core_db);
                    reply.send(res);
                    return;
                }
            }
        }

        flush_coalesced_progress(core_db, &mut pending_progress, &mut progress_replies);
    }

    flush_coalesced_progress(core_db, &mut pending_progress, &mut progress_replies);
    let _ = flush_durable_barrier(core_db);
}
