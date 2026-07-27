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

    /// Register an already-`Arc`-wrapped backend for a protocol kind.
    ///
    /// Use this when the caller (e.g. `bootstrap`) already holds an
    /// `Arc<T>` and wants the registry to share the SAME instance — avoiding
    /// a value `Clone` of `T` that would snapshot mutable state (e.g. fresh
    /// atomics) and silently diverge from the caller's copy.
    pub fn register_arc<T: DownloadBackend + 'static>(
        &mut self,
        kind: TaskKind,
        backend: Arc<T>,
    ) {
        let trait_obj: Arc<dyn DownloadBackend> = backend.clone();
        let any_obj: Arc<dyn Any + Send + Sync> = backend;
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
    ///
    /// Returns the first error encountered (settings are applied to all backends
    /// even if one errors, but the caller receives the first failure).
    pub async fn update_all_settings(&self, settings: &AppSettings) -> Result<(), DownloadError> {
        let mut first_err: Option<DownloadError> = None;
        for backend in self.iter() {
            if let Err(e) = backend.update_settings(settings).await {
                tracing::warn!("failed to update settings for a backend: {e}");
                if first_err.is_none() {
                    first_err = Some(e);
                }
            }
        }
        if let Some(e) = first_err {
            Err(e)
        } else {
            Ok(())
        }
    }

    /// Gracefully shut down all registered backends.
    pub async fn shutdown_all(&self) {
        for backend in self.iter() {
            backend.shutdown().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use async_trait::async_trait;
    use uuid::Uuid;

    use crate::error::DownloadError;
    use crate::protocol::DownloadBackend;
    use crate::types::{
        AppSettings, DownloadSnapshot, DownloadState, DownloadSummary, StartDownloadRequest,
        TaskId, TaskKind, ThreadMode,
    };

    // ── Mocks ──────────────────────────────────────────────────────────────

    /// Mock backend with call counters on list/update_settings/shutdown.
    struct MockBackend {
        name: &'static str,
        list_calls: Arc<AtomicUsize>,
        settings_calls: Arc<AtomicUsize>,
        shutdown_calls: Arc<AtomicUsize>,
        summaries: Vec<DownloadSummary>,
        list_error_msg: Option<&'static str>,
    }

    impl MockBackend {
        fn new(name: &'static str) -> Self {
            Self {
                name,
                list_calls: Arc::new(AtomicUsize::new(0)),
                settings_calls: Arc::new(AtomicUsize::new(0)),
                shutdown_calls: Arc::new(AtomicUsize::new(0)),
                summaries: Vec::new(),
                list_error_msg: None,
            }
        }

        fn with_summaries(summaries: Vec<DownloadSummary>) -> Self {
            Self {
                name: "mock",
                list_calls: Arc::new(AtomicUsize::new(0)),
                settings_calls: Arc::new(AtomicUsize::new(0)),
                shutdown_calls: Arc::new(AtomicUsize::new(0)),
                summaries,
                list_error_msg: None,
            }
        }

        fn with_list_error(name: &'static str) -> Self {
            Self {
                name,
                list_calls: Arc::new(AtomicUsize::new(0)),
                settings_calls: Arc::new(AtomicUsize::new(0)),
                shutdown_calls: Arc::new(AtomicUsize::new(0)),
                summaries: Vec::new(),
                list_error_msg: Some("simulated"),
            }
        }
    }

    #[async_trait]
    impl DownloadBackend for MockBackend {
        async fn start(&self, _: StartDownloadRequest) -> Result<TaskId, DownloadError> {
            unimplemented!()
        }
        async fn pause(&self, _: &TaskId) -> Result<DownloadSnapshot, DownloadError> {
            unimplemented!()
        }
        async fn resume(&self, _: &TaskId) -> Result<DownloadSnapshot, DownloadError> {
            unimplemented!()
        }
        async fn cancel(&self, _: &TaskId) -> Result<DownloadSnapshot, DownloadError> {
            unimplemented!()
        }
        async fn remove(&self, _: &TaskId) -> Result<DownloadSnapshot, DownloadError> {
            unimplemented!()
        }
        async fn purge(&self, _: &TaskId) -> Result<DownloadSnapshot, DownloadError> {
            unimplemented!()
        }
        async fn open_in_explorer(&self, _: &TaskId) -> Result<(), DownloadError> {
            unimplemented!()
        }
        async fn status(&self, _: &TaskId) -> Result<DownloadSnapshot, DownloadError> {
            unimplemented!()
        }
        async fn list(&self) -> Result<Vec<DownloadSummary>, DownloadError> {
            self.list_calls.fetch_add(1, Ordering::Relaxed);
            if let Some(msg) = self.list_error_msg {
                return Err(DownloadError::Internal(msg.into()));
            }
            Ok(self.summaries.clone())
        }
        async fn update_settings(&self, _: &AppSettings) -> Result<(), DownloadError> {
            self.settings_calls.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
        async fn shutdown(&self) {
            self.shutdown_calls.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// A second backend type — used to test that `get_typed` returns `None`
    /// when no backend of the requested type is registered.
    struct OtherMock;

    #[async_trait]
    impl DownloadBackend for OtherMock {
        async fn start(&self, _: StartDownloadRequest) -> Result<TaskId, DownloadError> {
            unimplemented!()
        }
        async fn pause(&self, _: &TaskId) -> Result<DownloadSnapshot, DownloadError> {
            unimplemented!()
        }
        async fn resume(&self, _: &TaskId) -> Result<DownloadSnapshot, DownloadError> {
            unimplemented!()
        }
        async fn cancel(&self, _: &TaskId) -> Result<DownloadSnapshot, DownloadError> {
            unimplemented!()
        }
        async fn remove(&self, _: &TaskId) -> Result<DownloadSnapshot, DownloadError> {
            unimplemented!()
        }
        async fn purge(&self, _: &TaskId) -> Result<DownloadSnapshot, DownloadError> {
            unimplemented!()
        }
        async fn open_in_explorer(&self, _: &TaskId) -> Result<(), DownloadError> {
            unimplemented!()
        }
        async fn status(&self, _: &TaskId) -> Result<DownloadSnapshot, DownloadError> {
            unimplemented!()
        }
        async fn list(&self) -> Result<Vec<DownloadSummary>, DownloadError> {
            Ok(Vec::new())
        }
        async fn update_settings(&self, _: &AppSettings) -> Result<(), DownloadError> {
            Ok(())
        }
        async fn shutdown(&self) {}
    }

    // ── Helpers ───────────────────────────────────────────────────────────

    fn make_http_task_id() -> TaskId {
        TaskId::Http(Uuid::from_u128(1))
    }

    fn make_bt_task_id() -> TaskId {
        TaskId::Bt(
            irontide::core::Id20::from_hex("0000000000000000000000000000000000000000")
                .expect("valid 40-char hex for Id20"),
        )
    }

    fn make_summary(created_at_ms: u64) -> DownloadSummary {
        DownloadSummary {
            id: String::new(),
            kind: TaskKind::Http,
            state: DownloadState::Queued,
            url: String::new(),
            file_name: String::new(),
            destination_path: String::new(),
            total_bytes: None,
            downloaded_bytes: 0,
            connection_count: 0,
            thread_mode: ThreadMode::Fixed,
            requested_thread_count: None,
            desired_thread_count: None,
            allocated_thread_count: None,
            adaptive_profile: None,
            thread_note: None,
            speed_bytes_per_second: None,
            eta_seconds: None,
            uploaded_bytes: None,
            upload_speed_bytes_per_second: None,
            peer_count: None,
            upload_status: None,
            info_hash: None,
            error: None,
            cdn_accelerated: false,
            cdn_node_ip: None,
            created_at_ms,
            priority: crate::types::Priority::Normal,
            seed_count: None,
            leech_count: None,
            download_limit_bps: None,
            upload_limit_bps: None,
            chunks: vec![],
            mirror_url: None,
        }
    }

    // ── Tests ─────────────────────────────────────────────────────────────

    #[test]
    fn new_registry_is_empty() {
        let reg = BackendRegistry::new();
        assert_eq!(reg.iter().count(), 0);
        assert!(reg.dispatch(&make_http_task_id()).is_err());
        assert!(reg.by_kind(TaskKind::Http).is_err());
    }

    #[test]
    fn register_and_dispatch_by_kind() {
        let mut reg = BackendRegistry::new();
        reg.register(TaskKind::Http, MockBackend::new("http"));
        assert!(reg.dispatch(&make_http_task_id()).is_ok());
        assert!(reg.by_kind(TaskKind::Http).is_ok());
    }

    #[test]
    fn register_two_backends_dispatch_routes_by_kind() {
        let mut reg = BackendRegistry::new();
        reg.register(TaskKind::Http, MockBackend::new("http-be"));
        reg.register(TaskKind::Bt, MockBackend::new("bt-be"));

        assert!(reg.dispatch(&make_http_task_id()).is_ok());
        assert!(reg.dispatch(&make_bt_task_id()).is_ok());
        assert!(reg.by_kind(TaskKind::Http).is_ok());
        assert!(reg.by_kind(TaskKind::Bt).is_ok());
    }

    #[tokio::test]
    async fn register_arc_shares_same_instance() {
        let mut reg = BackendRegistry::new();

        let shared = Arc::new(MockBackend::new("shared"));
        let saved_call_counter = shared.list_calls.clone();

        reg.register_arc(TaskKind::Http, shared);

        let typed = reg.get_typed::<MockBackend>().expect("typed backend");
        assert_eq!(typed.name, "shared");

        // Calling `list()` through the registry dispatch increments
        // the **same** atomic counter (proof of identity sharing).
        reg.dispatch(&make_http_task_id())
            .unwrap()
            .list()
            .await
            .unwrap();

        assert_eq!(saved_call_counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn dispatch_unregistered_kind_errors() {
        let reg = BackendRegistry::new();
        let result = reg.dispatch(&make_http_task_id());
        match result {
            Ok(_) => panic!("expected error, got Ok"),
            Err(e) => assert!(
                matches!(e, DownloadError::Internal(_)),
                "expected DownloadError::Internal, got: {e}",
            ),
        }
        let result = reg.by_kind(TaskKind::Http);
        match result {
            Ok(_) => panic!("expected error, got Ok"),
            Err(e) => assert!(
                matches!(e, DownloadError::Internal(_)),
                "expected DownloadError::Internal, got: {e}",
            ),
        }
    }

    #[test]
    fn get_typed_returns_concrete_backend() {
        let mut reg = BackendRegistry::new();
        reg.register(TaskKind::Http, MockBackend::new("my-be"));
        let typed = reg.get_typed::<MockBackend>().expect("get_typed");
        assert_eq!(typed.name, "my-be");
    }

    #[test]
    fn get_typed_wrong_type_returns_none() {
        let mut reg = BackendRegistry::new();
        reg.register(TaskKind::Http, OtherMock);
        assert!(reg.get_typed::<MockBackend>().is_none());
    }

    #[tokio::test]
    async fn iter_preserves_registration_order() {
        let mut reg = BackendRegistry::new();

        let s1 = vec![make_summary(100)];
        let s2 = vec![make_summary(200)];
        reg.register(TaskKind::Http, MockBackend::with_summaries(s1));
        reg.register(TaskKind::Bt, MockBackend::with_summaries(s2));

        let backends: Vec<&dyn DownloadBackend> = reg.iter().collect();
        assert_eq!(backends.len(), 2);

        let r0 = backends[0].list().await.unwrap();
        let r1 = backends[1].list().await.unwrap();
        assert_eq!(r0[0].created_at_ms, 100);
        assert_eq!(r1[0].created_at_ms, 200);
    }

    #[tokio::test]
    async fn list_all_merges_and_sorts_by_created_at_descending() {
        let mut reg = BackendRegistry::new();

        let summaries_b = vec![make_summary(300), make_summary(100)]; // back-end B
        let summaries_a = vec![make_summary(200)]; // back-end A

        reg.register(TaskKind::Http, MockBackend::with_summaries(summaries_a));
        reg.register(TaskKind::Bt, MockBackend::with_summaries(summaries_b));

        let all = reg.list_all().await;
        assert_eq!(all.len(), 3);
        // Must be sorted by created_at_ms DESC: 300, 200, 100
        assert_eq!(all[0].created_at_ms, 300);
        assert_eq!(all[1].created_at_ms, 200);
        assert_eq!(all[2].created_at_ms, 100);
    }

    #[tokio::test]
    async fn update_all_settings_broadcasts_to_every_backend() {
        let mut reg = BackendRegistry::new();

        let mock_a = MockBackend::new("a");
        let mock_b = MockBackend::new("b");
        let calls_a = mock_a.settings_calls.clone();
        let calls_b = mock_b.settings_calls.clone();

        reg.register(TaskKind::Http, mock_a);
        reg.register(TaskKind::Bt, mock_b);

        let settings = AppSettings::default();
        reg.update_all_settings(&settings).await.unwrap();

        assert_eq!(calls_a.load(Ordering::Relaxed), 1);
        assert_eq!(calls_b.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn shutdown_all_invokes_shutdown_on_every_backend() {
        let mut reg = BackendRegistry::new();

        let mock_a = MockBackend::new("a");
        let mock_b = MockBackend::new("b");
        let calls_a = mock_a.shutdown_calls.clone();
        let calls_b = mock_b.shutdown_calls.clone();

        reg.register(TaskKind::Http, mock_a);
        reg.register(TaskKind::Bt, mock_b);

        reg.shutdown_all().await;

        assert_eq!(calls_a.load(Ordering::Relaxed), 1);
        assert_eq!(calls_b.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn list_all_skips_backends_that_error_and_continues() {
        let mut reg = BackendRegistry::new();

        // Mock that always errors on list()
        let failing = MockBackend::with_list_error("failing");
        // Mock that returns a summary
        let ok = MockBackend::with_summaries(vec![make_summary(42)]);

        reg.register(TaskKind::Http, failing);
        reg.register(TaskKind::Bt, ok);

        let all = reg.list_all().await;
        // Only the successful summary should appear (no panic, no error item)
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].created_at_ms, 42);
    }

    #[test]
    fn register_replaces_existing_kind() {
        let mut reg = BackendRegistry::new();

        reg.register(TaskKind::Http, MockBackend::new("first"));
        reg.register(TaskKind::Http, MockBackend::new("second"));

        // dispatch uses by_kind HashMap — the last registration wins
        assert!(reg.dispatch(&make_http_task_id()).is_ok());
        let typed = reg.get_typed::<MockBackend>().expect("get_typed");
        assert_eq!(typed.name, "second");
    }
}
