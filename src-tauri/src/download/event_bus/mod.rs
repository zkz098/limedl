//! EventBus — unified publish/subscribe event bus for all download subsystems.
//!
//! Replaces the dual `broadcast::channel<String>` + `app_handle.emit()` pattern
//! with a single typed event bus. Subscribers receive strongly-typed events;
//! Tauri frontend emission is handled as a built-in subscriber.

use std::sync::Arc;
use parking_lot::RwLock;
use tokio::sync::broadcast;
use tauri::Emitter;

// ── Event types ──────────────────────────────────────────────────────────

/// Events published by download subsystems.
/// All payloads implement Serialize for Tauri IPC compatibility.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
pub(crate) enum DownloadEvent {
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
}

// ── EventBus ──────────────────────────────────────────────────────────────

/// Central event bus. Clone is cheap (Arc).
pub(crate) struct EventBus {
    tx: broadcast::Sender<DownloadEvent>,
    /// Optional Tauri AppHandle for frontend event emission.
    app_handle: Arc<RwLock<Option<tauri::AppHandle>>>,
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            app_handle: self.app_handle.clone(),
        }
    }
}

impl EventBus {
    /// Create a new EventBus with the given channel capacity.
    pub(crate) fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self {
            tx,
            app_handle: Arc::new(RwLock::new(None)),
        }
    }

    /// Inject the Tauri AppHandle so events are also emitted to the frontend.
    pub(crate) fn set_app_handle(&self, handle: tauri::AppHandle) {
        *self.app_handle.write() = Some(handle);
    }

    /// Publish an event to all subscribers AND emit to Tauri frontend.
    /// This is the primary API — callers don't need to know about frontend vs internal.
    pub(crate) fn publish(&self, event: DownloadEvent) {
        // Emit to Tauri frontend if handle is available
        if let Some(ref handle) = *self.app_handle.read() {
            match &event {
                DownloadEvent::Updated { id: _, summary_json } => {
                    let _ = handle.emit("download-updated", summary_json);
                }
                DownloadEvent::Progress { id: _, progress_json } => {
                    let _ = handle.emit("download-progress", progress_json);
                }
                DownloadEvent::Aria2Notification { event_name, gid } => {
                    let _ = handle.emit(event_name, gid);
                }
            }
        }
        // Broadcast to internal subscribers (Aria2 RPC, BT, etc.)
        let _ = self.tx.send(event);
    }

    /// Subscribe to all events. Returns a receiver for async iteration.
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<DownloadEvent> {
        self.tx.subscribe()
    }

    /// Returns the number of active receivers.
    pub(crate) fn receiver_count(&self) -> usize {
        self.tx.receiver_count()
    }
}
