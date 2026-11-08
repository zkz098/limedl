//! EventBus — unified publish/subscribe event bus for all download subsystems.
//!
//! Splits events into two isolated broadcast channels:
//! - **Lifecycle channel** (`Updated`, `Aria2Notification`, `CdnComplete`,
//!   `Warning`): low-frequency state transitions and notifications that must
//!   not be dropped by high-rate progress floods.
//! - **Progress channel** (`Progress`, `CdnProgress`): high-frequency telemetry
//!   ticks where skipping stale intermediate ticks on lag is benign.

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::{RecvError, TryRecvError};

use crate::types::{DownloadProgress, DownloadSummary};

// ── Event types ──────────────────────────────────────────────────────────

/// Events published by download subsystems.
///
/// Payloads are strongly-typed engine summaries, not wire JSON: the only
/// consumers are in-process (the desktop client and the Aria2 RPC bridge), so
/// the old `serde_json::Value` envelopes were removed. Serialize the payload at
/// the boundary that actually needs JSON (aria2 conversion, SQLite columns).
#[derive(Debug, Clone)]
pub enum DownloadEvent {
    /// A download task was added, changed state or removed.
    ///
    /// Boxed: `DownloadSummary` is the largest payload carried on the bus and
    /// the enum is cloned per receiver, so keeping it inline inflates every
    /// event (and the 8192-slot channel).
    Updated { summary: Box<DownloadSummary> },
    /// High-frequency progress update (bytes/speed).
    Progress { progress: DownloadProgress },
    /// Aria2-compatible event notification, consumed by the Aria2 RPC server.
    Aria2Notification { event_name: String, gid: String },
    /// CDN speed test progress update.
    CdnProgress {
        phase: String,
        current: u64,
        total: u64,
    },
    /// CDN speed test completed.
    CdnComplete {
        state: String,
        active_ip: Option<String>,
        active_speed_mbps: Option<f64>,
    },
    /// A warning or informational message for a specific download.
    Warning { id: String, message: String },
}

impl DownloadEvent {
    /// Returns `true` for high-frequency telemetry events (`Progress`,
    /// `CdnProgress`) that are routed over the dedicated progress channel.
    pub fn is_high_frequency(&self) -> bool {
        matches!(
            self,
            DownloadEvent::Progress { .. } | DownloadEvent::CdnProgress { .. }
        )
    }
}

// ── EventReceiver ─────────────────────────────────────────────────────────

/// Combined subscriber over both the lifecycle and high-frequency progress channels.
///
/// Prioritizes lifecycle events (`Updated`, `Aria2Notification`, `CdnComplete`,
/// `Warning`) over ephemeral progress ticks (`Progress`, `CdnProgress`), and
/// isolates their ring buffers so a burst of progress ticks cannot cause
/// [`RecvError::Lagged`] on lifecycle transitions.
pub struct EventReceiver {
    lifecycle_rx: broadcast::Receiver<DownloadEvent>,
    progress_rx: broadcast::Receiver<DownloadEvent>,
}

impl EventReceiver {
    /// Split into the raw `(lifecycle_rx, progress_rx)` broadcast receivers.
    pub fn into_parts(
        self,
    ) -> (
        broadcast::Receiver<DownloadEvent>,
        broadcast::Receiver<DownloadEvent>,
    ) {
        (self.lifecycle_rx, self.progress_rx)
    }

    /// Receive the next event, polling lifecycle events with higher priority
    /// than progress ticks.
    pub async fn recv(&mut self) -> Result<DownloadEvent, RecvError> {
        tokio::select! {
            biased;
            res = self.lifecycle_rx.recv() => res,
            res = self.progress_rx.recv() => res,
        }
    }

    /// Attempt to receive an available event without waiting, checking the
    /// lifecycle channel before the progress channel.
    pub fn try_recv(&mut self) -> Result<DownloadEvent, TryRecvError> {
        match self.lifecycle_rx.try_recv() {
            Err(TryRecvError::Empty) => self.progress_rx.try_recv(),
            other => other,
        }
    }
}

// ── EventBus ──────────────────────────────────────────────────────────────

/// Central event bus. Clone is cheap (`broadcast::Sender` is internally ref-counted).
pub struct EventBus {
    lifecycle_tx: broadcast::Sender<DownloadEvent>,
    progress_tx: broadcast::Sender<DownloadEvent>,
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            lifecycle_tx: self.lifecycle_tx.clone(),
            progress_tx: self.progress_tx.clone(),
        }
    }
}

impl EventBus {
    /// Create a new `EventBus` with the given per-channel capacity.
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let (lifecycle_tx, _) = broadcast::channel(cap);
        let (progress_tx, _) = broadcast::channel(cap);
        Self {
            lifecycle_tx,
            progress_tx,
        }
    }

    /// Publish an event to all subscribers on the appropriate channel.
    /// This is the primary API — callers don't need to know about subscribers.
    pub fn publish(&self, event: DownloadEvent) {
        let tx = if event.is_high_frequency() {
            &self.progress_tx
        } else {
            &self.lifecycle_tx
        };
        if let Err(tokio::sync::broadcast::error::SendError(_)) = tx.send(event) {
            tracing::warn!("EventBus publish dropped: no active subscribers");
        }
    }

    /// Subscribe to all events (both lifecycle and progress channels).
    pub fn subscribe(&self) -> EventReceiver {
        EventReceiver {
            lifecycle_rx: self.lifecycle_tx.subscribe(),
            progress_rx: self.progress_tx.subscribe(),
        }
    }

    /// Subscribe only to reliable lifecycle events (`Updated`,
    /// `Aria2Notification`, `CdnComplete`, `Warning`).
    ///
    /// High-frequency `Progress` and `CdnProgress` events never enter this
    /// receiver's buffer.
    pub fn subscribe_lifecycle(&self) -> broadcast::Receiver<DownloadEvent> {
        self.lifecycle_tx.subscribe()
    }

    /// Subscribe only to high-frequency progress events (`Progress`,
    /// `CdnProgress`).
    pub fn subscribe_progress(&self) -> broadcast::Receiver<DownloadEvent> {
        self.progress_tx.subscribe()
    }
}

#[cfg(test)]
mod tests;
