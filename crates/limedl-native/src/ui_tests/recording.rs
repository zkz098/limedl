//! A recording [`DownloadBackend`]: the UI's blast radius, made assertable.
//!
//! The UI tests care about *what the UI asks the engine to do* — whether the
//! trash button only removes the row or also deletes the file on disk, whether a
//! batch action touches the selection only, whether the explorer button uses the
//! task's own id. A recording backend answers exactly that and downloads
//! nothing, so the assertions stay synchronous and hermetic.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use parking_lot::Mutex;

use limedl_core::error::{DownloadError, Result as CoreResult};
use limedl_core::protocol::DownloadBackend;
use limedl_core::types::{
    AppSettings, ChecksumMode, DownloadSnapshot, DownloadSummary, Priority, StartDownloadRequest,
    TaskId, TaskKind,
};

/// The parts of a [`StartDownloadRequest`] the *dialog* is responsible for.
///
/// Recorded instead of the whole request because `StartDownloadRequest` is not
/// `PartialEq` (and carries fields the UI never sets): the assertion is about
/// what the form collected, so that is what the fixture keeps. `kind` is absent
/// on purpose — `Dispatcher::start` is what classifies the URL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StartCall {
    pub url: String,
    pub destination_dir: String,
    pub file_name: Option<String>,
    pub checksum: Option<ChecksumMode>,
    pub expected_checksum: Option<String>,
    pub selected_file_indices: Option<Vec<usize>>,
}

/// One call the UI made into the download engine, with the task it named.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CoreCall {
    /// A new task the UI asked for, with the payload it built.
    Start(StartCall),
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
/// [`TaskKind::Http`] and every task action the UI spawns lands in
/// [`RecordingBackend::calls`].
///
/// Mutating calls return `NotFound` on purpose: the fixtures seeded through
/// [`RecordingBackend::set_tasks`] are only there for the UI to read, and
/// keeping the mutation failing means no `DownloadEvent::Updated` is published
/// behind the test's back. It also makes the *failure* path reachable — the row
/// is dropped optimistically, the backend rejects the action, and the list has
/// to be resynchronized (see the rollback scenario in `async_contracts.rs`).
///
/// [`RecordingBackend::start`] is the exception: it answers `Ok` with a
/// synthetic id, because the success path is what clears the URL and closes the
/// dialog, and nothing else can reach it without a live backend. It publishes no
/// event either: `Dispatcher::start` emits one only when `status()` succeeds, and
/// `status()` is recorded as a failure here.
pub(crate) struct RecordingBackend {
    calls: Mutex<Vec<CoreCall>>,
    tasks: Mutex<Vec<DownloadSummary>>,
    /// Numbers the synthetic ids `start` hands out, so they stay distinct from
    /// the ones the fixtures seed.
    next_start: AtomicUsize,
}

impl RecordingBackend {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            tasks: Mutex::new(Vec::new()),
            next_start: AtomicUsize::new(1),
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

    /// Every `start` the UI issued, in order, with the payload it built.
    pub fn starts(&self) -> Vec<StartCall> {
        self.calls()
            .iter()
            .filter_map(|call| match call {
                CoreCall::Start(start) => Some(start.clone()),
                _ => None,
            })
            .collect()
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
        TaskKind::Http => format!("http:{raw}"),
        _ => format!("bt:{raw}"),
    }
}

#[async_trait]
impl DownloadBackend for RecordingBackend {
    async fn start(&self, request: StartDownloadRequest) -> CoreResult<TaskId> {
        let start = StartCall {
            url: request.url,
            destination_dir: request.destination_dir,
            file_name: request.file_name,
            checksum: request.checksum,
            expected_checksum: request.expected_checksum,
            selected_file_indices: request.selected_file_indices,
        };
        self.calls.lock().push(CoreCall::Start(start));

        // A valid uuid, but not one the fixtures seed: an assertion on the ids
        // the UI created must not collide with `http_wire(1..)`.
        let n = self.next_start.fetch_add(1, Ordering::Relaxed);
        TaskId::from_wire_string(&format!("http:00000000-0000-4000-8000-00000000c{n:03}"))
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
