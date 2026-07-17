use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

use super::protocol::DownloadBackend;
use super::types::{TaskId, TaskKind};

/// Owns all protocol backends and provides typed + trait-object access.
pub(crate) struct BackendRegistry {
    by_kind: HashMap<TaskKind, Arc<dyn DownloadBackend>>,
    by_type: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
    all: Vec<(TaskKind, Arc<dyn DownloadBackend>)>,
}

impl BackendRegistry {
    pub(crate) fn new() -> Self {
        Self {
            by_kind: HashMap::new(),
            by_type: HashMap::new(),
            all: Vec::new(),
        }
    }

    /// Register a backend for a protocol kind.
    pub(crate) fn register<T: DownloadBackend + 'static>(&mut self, kind: TaskKind, backend: T) {
        let arc: Arc<T> = Arc::new(backend);
        let trait_obj: Arc<dyn DownloadBackend> = arc.clone();
        let any_obj: Arc<dyn Any + Send + Sync> = arc;
        self.by_kind.insert(kind, trait_obj.clone());
        self.by_type.insert(TypeId::of::<T>(), any_obj);
        self.all.push((kind, trait_obj));
    }

    /// Dispatch by task ID for common operations.
    pub(crate) fn dispatch(&self, task_id: &TaskId) -> &dyn DownloadBackend {
        let kind = task_id.kind();
        self.by_kind.get(&kind).map(|arc| arc.as_ref())
            .expect("BUG: unregistered protocol kind in BackendRegistry")
    }

    /// Get a backend for the given kind (trait-object reference).
    pub(crate) fn by_kind(&self, kind: TaskKind) -> &dyn DownloadBackend {
        self.by_kind.get(&kind).map(|arc| arc.as_ref())
            .expect("BUG: unregistered protocol kind")
    }

    /// Get a concrete backend reference for protocol-specific commands.
    pub(crate) fn get_typed<T: DownloadBackend + 'static>(&self) -> Option<&T> {
        self.by_type.get(&TypeId::of::<T>())?
            .downcast_ref::<T>()
    }

    /// Iterate all backends for list merge, settings broadcast, shutdown.
    pub(crate) fn iter(&self) -> impl Iterator<Item = &dyn DownloadBackend> {
        self.all.iter().map(|(_, backend)| backend.as_ref())
    }
}
