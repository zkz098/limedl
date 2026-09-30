//! EventBus — unified publish/subscribe event bus for all download subsystems.
//!
//! Pure broadcast channel. UI emission is handled by independent subscriber
//! tasks in the application layer (desktop client / WebSocket RPC adapter).

use tokio::sync::broadcast;

// ── Event types ──────────────────────────────────────────────────────────

/// Events published by download subsystems.
/// All payloads implement Serialize for IPC/wire compatibility.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
pub enum DownloadEvent {
    /// A download task state changed (started/paused/resumed/completed/error/removed).
    /// Payload is a DownloadSummary serialized as JSON value.
    Updated {
        id: String,
        summary_json: serde_json::Value,
    },
    /// High-frequency progress update (bytes/speed).
    Progress {
        id: String,
        progress_json: serde_json::Value,
    },
    /// BT-specific: aria2-compatible event notifications.
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
    /// Full state recovery — sent when a subscriber recovers from lag.
    /// Contains all current download summaries for atomic frontend state replacement.
    FullState {
        downloads: Vec<crate::types::DownloadSummary>,
    },
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
