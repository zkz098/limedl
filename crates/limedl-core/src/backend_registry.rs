use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

use super::error::DownloadError;
use super::protocol::DownloadBackend;
use super::types::{AppSettings, DownloadSummary, TaskId, TaskKind};

/// Owns all protocol backends and provides typed + trait-object access.
pub struct BackendRegistry {
    by_kind: HashMap<TaskKind, Arc<dyn DownloadBackend>>,
    by_type: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
    all: Vec<(TaskKind, Arc<dyn DownloadBackend>)>,
}

impl Default for BackendRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl BackendRegistry {
    pub fn new() -> Self {
        Self {
            by_kind: HashMap::new(),
            by_type: HashMap::new(),
            all: Vec::new(),
        }
    }

    /// Register a backend for a protocol kind.
    pub fn register<T: DownloadBackend + 'static>(&mut self, kind: TaskKind, backend: T) {
        let arc: Arc<T> = Arc::new(backend);
        let trait_obj: Arc<dyn DownloadBackend> = arc.clone();
        let any_obj: Arc<dyn Any + Send + Sync> = arc;
        self.by_kind.insert(kind, trait_obj.clone());
        self.by_type.insert(TypeId::of::<T>(), any_obj);
        self.all.push((kind, trait_obj));
    }

    /// Dispatch by task ID for common operations.
    pub fn dispatch(&self, task_id: &TaskId) -> Result<&dyn DownloadBackend, DownloadError> {
        let kind = task_id.kind();
        self.by_kind
            .get(&kind)
            .map(|arc| arc.as_ref())
            .ok_or(DownloadError::Internal("unregistered protocol kind".into()))
    }

    /// Get a backend for the given kind (trait-object reference).
    pub fn by_kind(&self, kind: TaskKind) -> Result<&dyn DownloadBackend, DownloadError> {
        self.by_kind
            .get(&kind)
            .map(|arc| arc.as_ref())
            .ok_or(DownloadError::Internal("unregistered protocol kind".into()))
    }

    /// Get a concrete backend reference for protocol-specific commands.
    pub fn get_typed<T: DownloadBackend + 'static>(&self) -> Option<&T> {
        self.by_type.get(&TypeId::of::<T>())?.downcast_ref::<T>()
    }

    /// Iterate all backends for list merge, settings broadcast, shutdown.
    pub fn iter(&self) -> impl Iterator<Item = &dyn DownloadBackend> {
        self.all.iter().map(|(_, backend)| backend.as_ref())
    }

    /// List all downloads from all backends, merged and sorted by creation time descending.
    pub async fn list_all(&self) -> Vec<DownloadSummary> {
        let mut all: Vec<DownloadSummary> = Vec::new();
        for backend in self.iter() {
            match backend.list().await {
                Ok(summaries) => all.extend(summaries),
                Err(e) => tracing::warn!("failed to list from backend: {e}"),
            }
        }
        all.sort_by_key(|s| std::cmp::Reverse(s.created_at_ms));
        all
    }

    /// Broadcast settings to all registered backends.
    pub async fn update_all_settings(&self, settings: &AppSettings) {
        for backend in self.iter() {
            if let Err(e) = backend.update_settings(settings).await {
                tracing::warn!("failed to update settings for a backend: {e}");
            }
        }
    }

    /// Gracefully shut down all registered backends.
    pub async fn shutdown_all(&self) {
        for backend in self.iter() {
            backend.shutdown().await;
        }
    }
}
