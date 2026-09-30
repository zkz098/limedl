//! A recording [`DownloadBackend`]: the UI's blast radius, made assertable.
//!
//! The UI tests care about *what the UI asks the engine to do* — whether the
//! trash button only removes the row or also deletes the file on disk, whether a
//! batch action touches the selection only, whether the explorer button uses the
//! task's own id. A recording backend answers exactly that and downloads
//! nothing, so the assertions stay synchronous and hermetic.

use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;

use limedl_core::error::{DownloadError, Result as CoreResult};
use limedl_core::protocol::DownloadBackend;
use limedl_core::types::{
    AppSettings, DownloadSnapshot, DownloadSummary, Priority, StartDownloadRequest, TaskId,
};

/// One call the UI made into the download engine, with the task it named.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CoreCall {
    Start,
    Pause(String),
    Resume(String),
    Cancel(String),
    /// Drops the task, **keeping** the downloaded file.
    Remove(String),
    /// Drops the task **and** deletes the file on disk.
    Purge(String),
    OpenInExplorer(String),
    Status(String),
    SetPriority(String, Priority),
    UpdateSettings,
}

/// A [`DownloadBackend`] that records instead of downloading. Register it for
/// [`limedl_core::types::TaskKind::Http`] and every task action the UI spawns
/// lands in [`RecordingBackend::calls`].
///
/// Mutating calls return `NotFound` on purpose: the fixtures seeded through
/// [`RecordingBackend::set_tasks`] are only there for the UI to read, and
/// keeping the mutation failing means no `DownloadEvent::Updated` is published
/// behind the test's back.
pub(crate) struct RecordingBackend {
    calls: Mutex<Vec<CoreCall>>,
    tasks: Mutex<Vec<DownloadSummary>>,
}

impl RecordingBackend {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            tasks: Mutex::new(Vec::new()),
        })
    }

    fn record(&self, call: CoreCall) -> CoreResult<DownloadSnapshot> {
        self.calls.lock().push(call);
        Err(DownloadError::NotFound)
    }

    /// Every call so far, in order.
    pub fn calls(&self) -> Vec<CoreCall> {
        self.calls.lock().clone()
    }

    /// Forget the calls recorded so far, so a test can assert on one interaction.
    pub fn clear(&self) {
        self.calls.lock().clear();
    }

    /// What `Dispatcher::list` reports (and therefore what the UI shows).
    pub fn set_tasks(&self, tasks: Vec<DownloadSummary>) {
        *self.tasks.lock() = tasks;
    }

    fn named(calls: &[CoreCall], pick: impl Fn(&CoreCall) -> Option<&String>) -> Vec<String> {
        calls.iter().filter_map(pick).cloned().collect()
    }

    pub fn pauses(&self) -> Vec<String> {
        Self::named(&self.calls(), |call| match call {
            CoreCall::Pause(id) => Some(id),
            _ => None,
        })
    }

    pub fn resumes(&self) -> Vec<String> {
        Self::named(&self.calls(), |call| match call {
            CoreCall::Resume(id) => Some(id),
            _ => None,
        })
    }

    pub fn removes(&self) -> Vec<String> {
        Self::named(&self.calls(), |call| match call {
            CoreCall::Remove(id) => Some(id),
            _ => None,
        })
    }

    pub fn purges(&self) -> Vec<String> {
        Self::named(&self.calls(), |call| match call {
            CoreCall::Purge(id) => Some(id),
            _ => None,
        })
    }

    pub fn explorer(&self) -> Vec<String> {
        Self::named(&self.calls(), |call| match call {
            CoreCall::OpenInExplorer(id) => Some(id),
            _ => None,
        })
    }

    pub fn priorities(&self) -> Vec<(String, Priority)> {
        self.calls()
            .iter()
            .filter_map(|call| match call {
                CoreCall::SetPriority(id, priority) => Some((id.clone(), *priority)),
                _ => None,
            })
            .collect()
    }
}

/// The wire form of a task id (`http:<uuid>`), which is what the UI records and
/// what [`limedl_core::types::TaskId::from_wire_string`] accepts.
pub(crate) fn wire(id: &TaskId) -> String {
    let raw = id.raw_id();
    match id.kind() {
        limedl_core::types::TaskKind::Http => format!("http:{raw}"),
        _ => format!("bt:{raw}"),
    }
}

#[async_trait]
impl DownloadBackend for RecordingBackend {
    async fn start(&self, _request: StartDownloadRequest) -> CoreResult<TaskId> {
        self.calls.lock().push(CoreCall::Start);
        Err(DownloadError::UnsupportedScheme)
    }

    async fn pause(&self, task_id: &TaskId) -> CoreResult<DownloadSnapshot> {
        self.record(CoreCall::Pause(wire(task_id)))
    }

    async fn resume(&self, task_id: &TaskId) -> CoreResult<DownloadSnapshot> {
        self.record(CoreCall::Resume(wire(task_id)))
    }

    async fn cancel(&self, task_id: &TaskId) -> CoreResult<DownloadSnapshot> {
        self.record(CoreCall::Cancel(wire(task_id)))
    }

    async fn remove(&self, task_id: &TaskId) -> CoreResult<DownloadSnapshot> {
        self.record(CoreCall::Remove(wire(task_id)))
    }

    async fn purge(&self, task_id: &TaskId) -> CoreResult<DownloadSnapshot> {
        self.record(CoreCall::Purge(wire(task_id)))
    }

    async fn open_in_explorer(&self, task_id: &TaskId) -> CoreResult<()> {
        self.calls
            .lock()
            .push(CoreCall::OpenInExplorer(wire(task_id)));
        Ok(())
    }

    async fn status(&self, task_id: &TaskId) -> CoreResult<DownloadSnapshot> {
        self.record(CoreCall::Status(wire(task_id)))
    }

    async fn list(&self) -> CoreResult<Vec<DownloadSummary>> {
        Ok(self.tasks.lock().clone())
    }

    async fn update_settings(&self, _settings: &AppSettings) -> CoreResult<()> {
        self.calls.lock().push(CoreCall::UpdateSettings);
        Ok(())
    }

    async fn set_priority(&self, task_id: &TaskId, priority: Priority) -> CoreResult<()> {
        self.calls
            .lock()
            .push(CoreCall::SetPriority(wire(task_id), priority));
        Ok(())
    }

    async fn shutdown(&self) {}
}
