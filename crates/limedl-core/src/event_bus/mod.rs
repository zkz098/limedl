//! EventBus — unified publish/subscribe event bus for all download subsystems.
//!
//! Pure broadcast channel carrying the engine's own typed payloads. UI emission
//! is handled by an independent subscriber task in the application layer (the
//! Slint desktop client); the Aria2 RPC adapter subscribes for its own event
//! notifications.

use tokio::sync::broadcast;

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

// ── EventBus ──────────────────────────────────────────────────────────────

/// Central event bus. Clone is cheap (broadcast::Sender is internally ref-counted).
pub struct EventBus {
    tx: broadcast::Sender<DownloadEvent>,
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}

impl EventBus {
    /// Create a new EventBus with the given channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Publish an event to all subscribers.
    /// This is the primary API — callers don't need to know about subscribers.
    pub fn publish(&self, event: DownloadEvent) {
        if let Err(tokio::sync::broadcast::error::SendError(_)) = self.tx.send(event) {
            tracing::warn!("EventBus publish dropped: no active subscribers");
        }
    }

    /// Subscribe to all events. Returns a receiver for async iteration.
    pub fn subscribe(&self) -> broadcast::Receiver<DownloadEvent> {
        self.tx.subscribe()
    }
}

#[cfg(test)]
mod tests;
