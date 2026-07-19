//! EventBus — unified publish/subscribe event bus for all download subsystems.
//!
//! Pure broadcast channel. Tauri frontend emission is handled by an independent
//! subscriber task in the application layer.

use tokio::sync::broadcast;

// ── Event types ──────────────────────────────────────────────────────────

/// Events published by download subsystems.
/// All payloads implement Serialize for Tauri IPC compatibility.
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
    Aria2Notification {
        event_name: String,
        gid: String,
    },
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
        let _ = self.tx.send(event);
    }

    /// Subscribe to all events. Returns a receiver for async iteration.
    pub fn subscribe(&self) -> broadcast::Receiver<DownloadEvent> {
        self.tx.subscribe()
    }

}
