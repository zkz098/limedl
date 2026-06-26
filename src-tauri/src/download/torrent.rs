use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, Api, ManagedTorrent, Session,
    SessionOptions, SessionPersistenceConfig, TorrentStatsState, api::TorrentIdOrHash,
};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use uuid::Uuid;

use super::{
    error::{DownloadError, Result},
    lock_or_recover,
    types::{
        AppSettings, BtRuntimeStatus, BtSettings, BtUploadStatus, ChecksumMode, DownloadSnapshot,
        DownloadState, DownloadSummary, ProxyMode, StartDownloadRequest, TaskKind, ThreadMode,
    },
};

pub(super) const BT_PREFIX: &str = "bt:";
const BT_PENDING_PREFIX: &str = "bt:pending:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DownloadSourceKind {
    Http,
    Torrent,
    Metalink,
    Sftp,
}

pub struct TorrentManager {
    session: Arc<Session>,
    api: Api,
    state_dir: PathBuf,
    default_output_dir: PathBuf,
    bt_settings: Arc<Mutex<BtSettings>>,
    output_folders: Arc<Mutex<HashMap<usize, PathBuf>>>,
    pending: Arc<Mutex<HashMap<String, PendingTorrent>>>,
    event_tx: Arc<Mutex<Option<broadcast::Sender<String>>>>,
    last_states: Arc<Mutex<HashMap<usize, DownloadState>>>,
}

struct PendingTorrent {
    source: String,
    destination_dir: PathBuf,
    created_at_ms: u64,
    state: PendingTorrentState,
    join: Option<JoinHandle<()>>,
}

enum PendingTorrentState {
    Resolving,
    Paused,
    Failed(String),
    Added(usize),
}

impl TorrentManager {
    pub async fn new(state_dir: PathBuf, settings: &AppSettings) -> Result<Self> {
        fs::create_dir_all(&state_dir)?;
        let output_dir = state_dir.join("files");
        let persistence_dir = state_dir.join("session");
        fs::create_dir_all(&output_dir)?;
        fs::create_dir_all(&persistence_dir)?;

        let mut options = SessionOptions {
            disable_dht: !settings.bt.dht_enabled,
            fastresume: true,
            persistence: Some(SessionPersistenceConfig::Json {
                folder: Some(persistence_dir),
            }),
            ..SessionOptions::default()
        };

        if settings.proxy.mode == ProxyMode::Manual
            && settings
                .proxy
                .manual_url
                .to_ascii_lowercase()
                .starts_with("socks")
        {
            options.socks_proxy_url = Some(settings.proxy.manual_url.clone());
        }

        let session = match Session::new_with_opts(output_dir.clone(), options).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("BT session init failed: {e}, retrying with DHT disabled and no persistence");
                let fallback_options = SessionOptions {
                    disable_dht: true,
                    fastresume: false,
                    persistence: None,
                    ..SessionOptions::default()
                };
                Session::new_with_opts(output_dir.clone(), fallback_options)
                    .await
                    .map_err(|error| DownloadError::Torrent(error.to_string()))?
            }
        };

        let api = Api::new(session.clone(), None);

        Ok(Self {
            session,
            api,
            state_dir: state_dir.clone(),
            default_output_dir: output_dir,
            bt_settings: Arc::new(Mutex::new(settings.bt.clone())),
            output_folders: Arc::new(Mutex::new(HashMap::new())),
            pending: Arc::new(Mutex::new(HashMap::new())),
            event_tx: Arc::new(Mutex::new(None)),
            last_states: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn update_settings(&self, settings: &AppSettings) {
        *lock_or_recover(&self.bt_settings, "bt settings") = settings.bt.clone();
    }

    pub fn set_event_tx(&self, tx: broadcast::Sender<String>) {
        *lock_or_recover(&self.event_tx, "torrent event tx") = Some(tx);
    }

    pub async fn start(&self, request: StartDownloadRequest) -> Result<String> {
        let source = request.url.trim();
        if source.is_empty() {
            return Err(DownloadError::InvalidResponse(String::from(
                "torrent source is empty",
            )));
        }

        let destination_dir = PathBuf::from(request.destination_dir.trim());
        if destination_dir.as_os_str().is_empty() {
            return Err(DownloadError::InvalidResponse(String::from(
                "download destination directory is not set",
            )));
        }
        if !destination_dir.is_absolute() {
            return Err(DownloadError::InvalidResponse(String::from(
                "download destination directory must be an absolute path",
            )));
        }
        fs::create_dir_all(&destination_dir)?;

        let pending_id = pending_bt_task_id();
        let bt_settings = self
            .bt_settings
            .lock()
            .unwrap_or_else(|poisoned| {
                tracing::warn!("bt settings lock poisoned, recovering with inner state");
                poisoned.into_inner()
            })
            .clone();
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| {
                tracing::warn!("pending torrents lock poisoned, recovering with inner state");
                poisoned.into_inner()
            })
            .insert(
                pending_id.clone(),
                PendingTorrent {
                    source: source.to_string(),
                    destination_dir: destination_dir.clone(),
                    created_at_ms: now_ms(),
                    state: PendingTorrentState::Resolving,
                    join: None,
                },
            );
        let join = self.spawn_add_torrent(
            pending_id.clone(),
            source.to_string(),
            destination_dir,
            bt_settings,
        );
        if let Some(task) = self
            .pending
            .lock()
            .unwrap_or_else(|poisoned| {
                tracing::warn!("pending torrents lock poisoned, recovering with inner state");
                poisoned.into_inner()
            })
            .get_mut(&pending_id)
        {
            task.join = Some(join);
        }

        Ok(pending_id)
    }

    fn spawn_add_torrent(
        &self,
        pending_id: String,
        source: String,
        destination_dir: PathBuf,
        bt_settings: BtSettings,
    ) -> JoinHandle<()> {
        let session = self.session.clone();
        let pending = self.pending.clone();
        let output_folders = self.output_folders.clone();

        tokio::spawn(async move {
            let result = add_torrent_to_session(&session, &source, &destination_dir, &bt_settings)
                .await
                .inspect(|&id| {
                    output_folders
                        .lock()
                        .unwrap_or_else(|poisoned| {
                            tracing::warn!(
                                "output folders lock poisoned, recovering with inner state"
                            );
                            poisoned.into_inner()
                        })
                        .insert(id, destination_dir);
                })
                .map_err(|error| error.to_string());

            let mut added_without_owner = None;
            {
                let mut pending = lock_or_recover(&pending, "pending torrents");
                if let Some(task) = pending.get_mut(&pending_id) {
                    task.join = None;
                    task.state = match result {
                        Ok(id) => PendingTorrentState::Added(id),
                        Err(error) => PendingTorrentState::Failed(error),
                    };
                } else if let Ok(id) = result {
                    added_without_owner = Some(id);
                }
            }

            if let Some(id) = added_without_owner {
                let _ = session.delete(TorrentIdOrHash::Id(id), false).await;
                output_folders
                    .lock()
                    .unwrap_or_else(|poisoned| {
                        tracing::warn!("output folders lock poisoned, recovering with inner state");
                        poisoned.into_inner()
                    })
                    .remove(&id);
            }
        })
    }

    async fn restart_pending(&self, pending_id: &str) -> Result<DownloadSnapshot> {
        let mut managed_id = None;
        let (source, destination_dir, created_at_ms) = {
            let mut pending = lock_or_recover(&self.pending, "pending torrents");
            let task = pending.get_mut(pending_id).ok_or(DownloadError::NotFound)?;

            match task.state {
                PendingTorrentState::Resolving => {
                    return Ok(self.pending_snapshot(pending_id, task));
                }
                PendingTorrentState::Added(id) => {
                    managed_id = Some(id);
                    (String::new(), PathBuf::new(), 0)
                }
                PendingTorrentState::Paused | PendingTorrentState::Failed(_) => (
                    task.source.clone(),
                    task.destination_dir.clone(),
                    task.created_at_ms,
                ),
            }
        };

        if let Some(id) = managed_id {
            return self.status_for_managed(pending_id, id).await;
        }

        let bt_settings = self
            .bt_settings
            .lock()
            .unwrap_or_else(|poisoned| {
                tracing::warn!("bt settings lock poisoned, recovering with inner state");
                poisoned.into_inner()
            })
            .clone();
        let join = self.spawn_add_torrent(
            pending_id.to_string(),
            source,
            destination_dir.clone(),
            bt_settings,
        );

        let mut pending = lock_or_recover(&self.pending, "pending torrents");
        let task = pending.get_mut(pending_id).ok_or(DownloadError::NotFound)?;
        task.state = PendingTorrentState::Resolving;
        task.join = Some(join);
        task.created_at_ms = created_at_ms;
        Ok(self.pending_snapshot(pending_id, task))
    }

    async fn pause_pending(&self, pending_id: &str) -> Result<DownloadSnapshot> {
        let managed_id = {
            let mut pending = lock_or_recover(&self.pending, "pending torrents");
            let task = pending.get_mut(pending_id).ok_or(DownloadError::NotFound)?;
            if let PendingTorrentState::Added(id) = task.state {
                Some(id)
            } else {
                if let Some(join) = task.join.take() {
                    join.abort();
                }
                task.state = PendingTorrentState::Paused;
                return Ok(self.pending_snapshot(pending_id, task));
            }
        };

        if let Some(id) = managed_id {
            let handle = self.get_handle(id)?;
            self.session
                .pause(&handle)
                .await
                .map_err(|error| DownloadError::Torrent(error.to_string()))?;
            return self.status_for_managed(pending_id, id).await;
        }

        Err(DownloadError::NotFound)
    }

    async fn cancel_pending(&self, pending_id: &str) -> Result<DownloadSnapshot> {
        let snapshot = self.status(pending_id).await?;
        self.delete_pending(pending_id, false).await?;
        Ok(DownloadSnapshot {
            state: DownloadState::Canceled,
            updated_at_ms: now_ms(),
            ..snapshot
        })
    }

    async fn remove_pending(
        &self,
        pending_id: &str,
        delete_files: bool,
    ) -> Result<DownloadSnapshot> {
        let snapshot = self.status(pending_id).await?;
        self.delete_pending(pending_id, delete_files).await?;
        Ok(snapshot)
    }
}

async fn add_torrent_to_session(
    session: &Arc<Session>,
    source: &str,
    destination_dir: &Path,
    bt_settings: &BtSettings,
) -> Result<usize> {
    let add = build_add_torrent(source)?;
    let options = AddTorrentOptions {
        output_folder: Some(destination_dir.to_string_lossy().to_string()),
        overwrite: true,
        trackers: tracker_list_entries(&bt_settings.tracker_list),
        ..AddTorrentOptions::default()
    };

    let id = match session
        .add_torrent(add, Some(options))
        .await
        .map_err(|error| DownloadError::Torrent(error.to_string()))?
    {
        AddTorrentResponse::Added(id, _) | AddTorrentResponse::AlreadyManaged(id, _) => id,
        AddTorrentResponse::ListOnly(_) => {
            return Err(DownloadError::Torrent(String::from(
                "torrent was opened in list-only mode",
            )));
        }
    };

    Ok(id)
}

impl TorrentManager {
    pub async fn pause(&self, download_id: &str) -> Result<DownloadSnapshot> {
        if is_pending_bt_task_id(download_id) {
            return self.pause_pending(download_id).await;
        }

        let id = parse_bt_task_id(download_id)?;
        let handle = self.get_handle(id)?;
        self.session
            .pause(&handle)
            .await
            .map_err(|error| DownloadError::Torrent(error.to_string()))?;
        self.status(download_id).await
    }

    pub async fn resume(&self, download_id: &str) -> Result<DownloadSnapshot> {
        if is_pending_bt_task_id(download_id) {
            return self.restart_pending(download_id).await;
        }

        let id = parse_bt_task_id(download_id)?;
        let handle = self.get_handle(id)?;
        self.session
            .unpause(&handle)
            .await
            .map_err(|error| DownloadError::Torrent(error.to_string()))?;
        self.status(download_id).await
    }

    pub async fn cancel(&self, download_id: &str) -> Result<DownloadSnapshot> {
        if is_pending_bt_task_id(download_id) {
            return self.cancel_pending(download_id).await;
        }

        let snapshot = self.status(download_id).await?;
        self.delete(download_id, false).await?;
        Ok(DownloadSnapshot {
            state: DownloadState::Canceled,
            updated_at_ms: now_ms(),
            ..snapshot
        })
    }

    pub async fn remove(&self, download_id: &str) -> Result<DownloadSnapshot> {
        if is_pending_bt_task_id(download_id) {
            return self.remove_pending(download_id, false).await;
        }

        let snapshot = self.status(download_id).await?;
        self.delete(download_id, false).await?;
        Ok(snapshot)
    }

    pub async fn purge(&self, download_id: &str) -> Result<DownloadSnapshot> {
        if is_pending_bt_task_id(download_id) {
            return self.remove_pending(download_id, true).await;
        }

        let snapshot = self.status(download_id).await?;
        self.delete(download_id, true).await?;
        Ok(snapshot)
    }

    pub async fn open_in_explorer(&self, download_id: &str) -> Result<()> {
        let snapshot = self.status(download_id).await?;
        let path = PathBuf::from(&snapshot.destination_path);
        if path.exists() {
            #[cfg(windows)]
            { Command::new("explorer").arg(&path).spawn()?; }
            return Ok(());
        }

        Err(DownloadError::Io(io::Error::new(
            io::ErrorKind::NotFound,
            "torrent download location does not exist",
        )))
    }

    pub async fn status(&self, download_id: &str) -> Result<DownloadSnapshot> {
        if is_pending_bt_task_id(download_id) {
            let managed_id = {
                let pending = lock_or_recover(&self.pending, "pending torrents");
                let task = pending.get(download_id).ok_or(DownloadError::NotFound)?;
                match task.state {
                    PendingTorrentState::Added(id) => Some(id),
                    _ => return Ok(self.pending_snapshot(download_id, task)),
                }
            };

            if let Some(id) = managed_id {
                return self.status_for_managed(download_id, id).await;
            }
        }

        let id = parse_bt_task_id(download_id)?;
        self.status_for_managed(download_id, id).await
    }

    pub async fn list(&self) -> Result<Vec<DownloadSummary>> {
        let handles = self.session.with_torrents(|torrents| {
            torrents
                .map(|(id, handle)| (id, handle.clone()))
                .collect::<Vec<_>>()
        });
        let mut pending_snapshots = Vec::new();
        let mut represented_ids = HashSet::new();
        {
            let pending = lock_or_recover(&self.pending, "pending torrents");
            for (pending_id, task) in pending.iter() {
                if let PendingTorrentState::Added(id) = task.state {
                    represented_ids.insert(id);
                }
                pending_snapshots
                    .push((pending_id.clone(), self.pending_snapshot(pending_id, task)));
            }
        }

        let mut summaries = Vec::with_capacity(handles.len() + pending_snapshots.len());
        for snapshot in pending_snapshots {
            let (pending_id, mut snapshot) = snapshot;
            if let Some(id) = self.pending_managed_id(&pending_id)
                && let Ok(managed) = self.status_for_managed(&pending_id, id).await
            {
                snapshot = managed;
            }
            summaries.push(DownloadSummary::from(&snapshot));
        }

        for (id, handle) in handles {
            if represented_ids.contains(&id) {
                continue;
            }
            let limit_reached = self.apply_upload_policy(&handle).await?;
            summaries.push(DownloadSummary::from(&self.snapshot_from_handle(
                id,
                &handle,
                limit_reached,
            )));
        }
        summaries.sort_by_key(|right| std::cmp::Reverse(right.created_at_ms));
        Ok(summaries)
    }

    pub fn runtime_status(&self) -> BtRuntimeStatus {
        let dht_enabled = lock_or_recover(&self.bt_settings, "bt settings").dht_enabled;
        let dht_nodes = self
            .api
            .api_dht_stats()
            .ok()
            .map(|stats| stats.routing_table_size);
        let session_stats = self.session.stats_snapshot();
        let upload_speed = mib_per_second_to_bytes_per_second(session_stats.upload_speed.mbps);
        let uploaded_bytes = session_stats.uploaded_bytes;

        let handles = self.session.with_torrents(|torrents| {
            torrents
                .map(|(_, handle)| handle.clone())
                .collect::<Vec<_>>()
        });
        let mut peer_count = 0;
        for handle in &handles {
            let stats = handle.stats();
            if let Some(live) = stats.live.as_ref() {
                peer_count += live.snapshot.peer_stats.live + live.snapshot.peer_stats.connecting;
            }
        }

        let pending_count = lock_or_recover(&self.pending, "pending torrents")
            .values()
            .filter(|task| {
                matches!(
                    task.state,
                    PendingTorrentState::Resolving | PendingTorrentState::Paused
                )
            })
            .count();
        let torrent_count = handles.len() + pending_count;
        let connected = peer_count > 0 || dht_nodes.unwrap_or(0) > 0 || upload_speed > 0.0;

        BtRuntimeStatus {
            connected,
            dht_enabled,
            dht_nodes,
            torrent_count,
            peer_count,
            upload_speed_bytes_per_second: (upload_speed > 0.0).then_some(upload_speed),
            uploaded_bytes,
            updated_at_ms: now_ms(),
        }
    }

    async fn status_for_managed(&self, download_id: &str, id: usize) -> Result<DownloadSnapshot> {
        let handle = self.get_handle(id)?;
        let limit_reached = self.apply_upload_policy(&handle).await?;
        let mut snapshot = self.snapshot_from_handle(id, &handle, limit_reached);
        snapshot.id = download_id.to_string();
        Ok(snapshot)
    }

    fn pending_managed_id(&self, pending_id: &str) -> Option<usize> {
        let pending = lock_or_recover(&self.pending, "pending torrents");
        pending.get(pending_id).and_then(|task| match task.state {
            PendingTorrentState::Added(id) => Some(id),
            _ => None,
        })
    }

    fn pending_snapshot(&self, pending_id: &str, task: &PendingTorrent) -> DownloadSnapshot {
        let now = now_ms();
        let (state, error) = match &task.state {
            PendingTorrentState::Resolving => (DownloadState::Queued, None),
            PendingTorrentState::Paused => (DownloadState::Paused, None),
            PendingTorrentState::Failed(error) => (DownloadState::Failed, Some(error.clone())),
            PendingTorrentState::Added(_) => (DownloadState::Queued, None),
        };

        DownloadSnapshot {
            id: pending_id.to_string(),
            kind: TaskKind::Bt,
            state,
            url: task.source.clone(),
            final_url: task.source.clone(),
            file_name: display_torrent_source(&task.source),
            destination_path: task.destination_dir.to_string_lossy().to_string(),
            temp_path: self.state_dir.to_string_lossy().to_string(),
            total_bytes: None,
            downloaded_bytes: 0,
            supports_ranges: false,
            connection_count: 0,
            thread_mode: ThreadMode::Fixed,
            requested_thread_count: None,
            desired_thread_count: None,
            allocated_thread_count: None,
            adaptive_profile: None,
            thread_note: Some(String::from("Resolving BT metadata in background")),
            checksum: None,
            checksum_mode: ChecksumMode::None,
            etag: None,
            last_modified: None,
            error,
            speed_bytes_per_second: None,
            eta_seconds: None,
            uploaded_bytes: Some(0),
            upload_speed_bytes_per_second: None,
            peer_count: Some(0),
            upload_status: Some(BtUploadStatus::Idle),
            info_hash: None,
            created_at_ms: task.created_at_ms,
            updated_at_ms: now,
            cdn_accelerated: false,
            chunks: vec![],
        }
    }

    fn get_handle(&self, id: usize) -> Result<Arc<ManagedTorrent>> {
        self.session
            .get(TorrentIdOrHash::Id(id))
            .ok_or(DownloadError::NotFound)
    }

    async fn delete(&self, download_id: &str, delete_files: bool) -> Result<()> {
        if is_pending_bt_task_id(download_id) {
            return self.delete_pending(download_id, delete_files).await;
        }

        let id = parse_bt_task_id(download_id)?;
        self.session
            .delete(TorrentIdOrHash::Id(id), delete_files)
            .await
            .map_err(|error| DownloadError::Torrent(error.to_string()))?;
        self.output_folders
            .lock()
            .unwrap_or_else(|poisoned| {
                tracing::warn!("output folders lock poisoned, recovering with inner state");
                poisoned.into_inner()
            })
            .remove(&id);
        Ok(())
    }

    async fn delete_pending(&self, pending_id: &str, delete_files: bool) -> Result<()> {
        let mut task = self
            .pending
            .lock()
            .unwrap_or_else(|poisoned| {
                tracing::warn!("pending torrents lock poisoned, recovering with inner state");
                poisoned.into_inner()
            })
            .remove(pending_id)
            .ok_or(DownloadError::NotFound)?;

        if let Some(join) = task.join.take() {
            join.abort();
        }

        if let PendingTorrentState::Added(id) = task.state {
            self.session
                .delete(TorrentIdOrHash::Id(id), delete_files)
                .await
                .map_err(|error| DownloadError::Torrent(error.to_string()))?;
            self.output_folders
                .lock()
                .unwrap_or_else(|poisoned| {
                    tracing::warn!("output folders lock poisoned, recovering with inner state");
                    poisoned.into_inner()
                })
                .remove(&id);
        }

        Ok(())
    }

    async fn apply_upload_policy(&self, handle: &Arc<ManagedTorrent>) -> Result<bool> {
        let stats = handle.stats();
        let settings = self
            .bt_settings
            .lock()
            .unwrap_or_else(|poisoned| {
                tracing::warn!("bt settings lock poisoned, recovering with inner state");
                poisoned.into_inner()
            })
            .clone();
        let limit_reached =
            upload_limit_reached(&settings, stats.uploaded_bytes, stats.progress_bytes);

        if settings.pause_upload_when_limit_reached
            && limit_reached
            && matches!(stats.state, TorrentStatsState::Live)
        {
            self.session
                .pause(handle)
                .await
                .map_err(|error| DownloadError::Torrent(error.to_string()))?;
        }

        Ok(settings.pause_upload_when_limit_reached && limit_reached)
    }
}

fn is_terminal(state: DownloadState) -> bool {
    matches!(state, DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled)
}

impl TorrentManager {
    fn snapshot_from_handle(
        &self,
        id: usize,
        handle: &Arc<ManagedTorrent>,
        upload_limit_reached: bool,
    ) -> DownloadSnapshot {
        let stats = handle.stats();
        let state = map_torrent_state(stats.state, stats.finished);
        let now = now_ms();
        let downloaded = stats.progress_bytes;
        let speed = stats
            .live
            .as_ref()
            .map(|live| mib_per_second_to_bytes_per_second(live.download_speed.mbps))
            .filter(|speed| *speed > 0.0);
        let info_hash = handle.info_hash().as_string();
        let name = handle
            .name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| format!("Torrent {id}"));
        let output_folder = self
            .api
            .api_torrent_details(TorrentIdOrHash::Id(id))
            .ok()
            .map(|details| details.output_folder)
            .or_else(|| {
                self.output_folders
                    .lock()
                    .unwrap_or_else(|poisoned| {
                        tracing::warn!("output folders lock poisoned, recovering with inner state");
                        poisoned.into_inner()
                    })
                    .get(&id)
                    .cloned()
                    .map(|path| path.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| self.default_output_dir.to_string_lossy().to_string());
        let peer_count = stats
            .live
            .as_ref()
            .map(|live| live.snapshot.peer_stats.live + live.snapshot.peer_stats.connecting);
        let peer_count = peer_count.unwrap_or(0);
        let upload_status = upload_status_from_stats(&stats, upload_limit_reached);
        let upload_speed = stats
            .live
            .as_ref()
            .map(|live| mib_per_second_to_bytes_per_second(live.upload_speed.mbps))
            .filter(|speed| *speed > 0.0);

        // Detect terminal state transitions and broadcast events
        {
            let mut last_states = lock_or_recover(&self.last_states, "torrent last states");
            let prev_state = last_states.get(&id).copied();
            if prev_state != Some(state) {
                last_states.insert(id, state);
                if let Some(ref tx) = *lock_or_recover(&self.event_tx, "torrent event tx") {
                    let gid = crate::download::aria2_rpc::internal_id_to_gid(&bt_task_id(id));
                    match state {
                        DownloadState::Completed => {
                            let _ = tx.send(build_event_json("aria2.onDownloadComplete", &gid));
                            let _ = tx.send(build_event_json("aria2.onBtDownloadComplete", &gid));
                        }
                        DownloadState::Failed => {
                            let _ = tx.send(build_event_json("aria2.onDownloadError", &gid));
                        }
                        _ => {}
                    }
                }
            }
        }

        DownloadSnapshot {
            id: bt_task_id(id),
            kind: TaskKind::Bt,
            state,
            url: info_hash.clone(),
            final_url: info_hash,
            file_name: name,
            destination_path: output_folder,
            temp_path: self.state_dir.to_string_lossy().to_string(),
            total_bytes: Some(stats.total_bytes),
            downloaded_bytes: downloaded,
            supports_ranges: false,
            connection_count: peer_count,
            thread_mode: ThreadMode::Fixed,
            requested_thread_count: None,
            desired_thread_count: None,
            allocated_thread_count: None,
            adaptive_profile: None,
            thread_note: Some(String::from("BT task managed by rqbit")),
            checksum: None,
            checksum_mode: ChecksumMode::None,
            etag: None,
            last_modified: None,
            error: stats.error,
            speed_bytes_per_second: if is_terminal(state) { None } else { speed },
            eta_seconds: if is_terminal(state) {
                None
            } else {
                estimate_eta(stats.total_bytes, downloaded, speed)
            },
            uploaded_bytes: Some(stats.uploaded_bytes),
            upload_speed_bytes_per_second: if is_terminal(state) { None } else { upload_speed },
            peer_count: Some(peer_count),
            upload_status: Some(upload_status),
            info_hash: Some(handle.info_hash().as_string()),
            created_at_ms: now,
            updated_at_ms: now,
            cdn_accelerated: false,
            chunks: vec![],
        }
    }
}

pub(super) fn classify_download_source(
    request: &StartDownloadRequest,
) -> Result<DownloadSourceKind> {
    if let Some(kind) = request.kind {
        return Ok(match kind {
            TaskKind::Http => DownloadSourceKind::Http,
            TaskKind::Bt => DownloadSourceKind::Torrent,
            TaskKind::Metalink => DownloadSourceKind::Metalink,
            TaskKind::Sftp => DownloadSourceKind::Sftp,
        });
    }

    let source = request.url.trim();
    let lower = source.to_ascii_lowercase();

    if lower.starts_with("magnet:") || lower.ends_with(".torrent") {
        return Ok(DownloadSourceKind::Torrent);
    }

    if lower.ends_with(".metalink") || lower.ends_with(".meta4") {
        return Ok(DownloadSourceKind::Metalink);
    }

    if lower.starts_with("sftp://") {
        return Ok(DownloadSourceKind::Sftp);
    }

    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Ok(DownloadSourceKind::Http);
    }

    let path = Path::new(source);
    if path.extension().and_then(|value| value.to_str()) == Some("torrent") {
        return Ok(DownloadSourceKind::Torrent);
    }

    if matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase()),
        Some(extension) if extension == "metalink" || extension == "meta4"
    ) {
        return Ok(DownloadSourceKind::Metalink);
    }

    Err(DownloadError::UnsupportedScheme)
}

fn is_pending_bt_task_id(download_id: &str) -> bool {
    download_id.starts_with(BT_PENDING_PREFIX)
}

pub(super) fn build_add_torrent(source: &str) -> Result<AddTorrent<'_>> {
    let lower = source.to_ascii_lowercase();
    if lower.starts_with("magnet:") || lower.starts_with("http://") || lower.starts_with("https://")
    {
        return Ok(AddTorrent::from_url(source));
    }

    AddTorrent::from_local_filename(source)
        .map_err(|error| DownloadError::Torrent(error.to_string()))
}

fn parse_bt_task_id(download_id: &str) -> Result<usize> {
    download_id
        .strip_prefix(BT_PREFIX)
        .ok_or(DownloadError::NotFound)?
        .parse::<usize>()
        .map_err(|_| DownloadError::NotFound)
}

fn bt_task_id(id: usize) -> String {
    format!("{BT_PREFIX}{id}")
}

fn pending_bt_task_id() -> String {
    format!("{BT_PENDING_PREFIX}{}", Uuid::new_v4())
}

fn display_torrent_source(source: &str) -> String {
    let trimmed = source.trim();
    if trimmed.to_ascii_lowercase().starts_with("magnet:") {
        return String::from("Resolving magnet link");
    }

    Path::new(trimmed)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| String::from("Torrent metadata"))
}

fn map_torrent_state(state: TorrentStatsState, finished: bool) -> DownloadState {
    if finished {
        return DownloadState::Completed;
    }

    match state {
        TorrentStatsState::Initializing => DownloadState::Queued,
        TorrentStatsState::Live => DownloadState::Downloading,
        TorrentStatsState::Paused => DownloadState::Paused,
        TorrentStatsState::Error => DownloadState::Failed,
    }
}

fn estimate_eta(total: u64, downloaded: u64, speed: Option<f64>) -> Option<u64> {
    let speed = speed?;
    if total <= downloaded || speed <= 0.0 {
        return None;
    }
    Some(((total - downloaded) as f64 / speed).ceil() as u64)
}

fn upload_limit_reached(settings: &BtSettings, uploaded: u64, downloaded: u64) -> bool {
    let bytes_reached = settings.upload_limit_bytes > 0 && uploaded >= settings.upload_limit_bytes;
    let ratio_reached = settings.upload_ratio_limit > 0.0
        && downloaded > 0
        && uploaded as f64 >= downloaded as f64 * settings.upload_ratio_limit;

    bytes_reached || ratio_reached
}

fn tracker_list_entries(tracker_list: &str) -> Option<Vec<String>> {
    let trackers = tracker_list
        .lines()
        .map(str::trim)
        .filter(|tracker| !tracker.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    (!trackers.is_empty()).then_some(trackers)
}

fn upload_status_from_stats(
    stats: &librqbit::api::TorrentStats,
    upload_limit_reached: bool,
) -> BtUploadStatus {
    if upload_limit_reached {
        return BtUploadStatus::PausedByLimit;
    }

    if matches!(stats.state, TorrentStatsState::Paused) {
        return BtUploadStatus::Paused;
    }

    let upload_speed = stats
        .live
        .as_ref()
        .map(|live| live.upload_speed.mbps)
        .unwrap_or(0.0);
    if upload_speed > 0.0 {
        return BtUploadStatus::Uploading;
    }

    BtUploadStatus::Idle
}

fn mib_per_second_to_bytes_per_second(value: f64) -> f64 {
    value * 1024.0 * 1024.0
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn build_event_json(method: &str, gid: &str) -> String {
    serde_json::to_string(&serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": [{"gid": gid}]
    }))
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use ntest::timeout;

    use super::{
        BT_PREFIX, DownloadSourceKind, build_add_torrent, classify_download_source,
        mib_per_second_to_bytes_per_second, upload_limit_reached,
    };
    use crate::download::types::{BtSettings, StartDownloadRequest, TaskId, TaskKind};

    type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

    fn request(url: &str, kind: Option<TaskKind>) -> StartDownloadRequest {
        StartDownloadRequest {
            kind,
            url: String::from(url),
            destination_dir: String::from("E:/tmp"),
            file_name: None,
            user_agent: None,
            thread_mode: None,
            thread_count: None,
            max_retries: None,
            checksum: None,
        }
    }

    #[timeout(30_000)]
    #[test]
    fn classifies_download_sources() -> TestResult {
        assert_eq!(
            classify_download_source(&request("https://example.com/file.zip", None))?,
            DownloadSourceKind::Http
        );
        assert_eq!(
            classify_download_source(&request("magnet:?xt=urn:btih:abc", None))?,
            DownloadSourceKind::Torrent
        );
        assert_eq!(
            classify_download_source(&request("https://example.com/file.torrent", None))?,
            DownloadSourceKind::Torrent
        );
        assert_eq!(
            classify_download_source(&request("https://example.com/file.meta4", None))?,
            DownloadSourceKind::Metalink
        );
        assert_eq!(
            classify_download_source(&request("sftp://example.com/file.zip", None))?,
            DownloadSourceKind::Sftp
        );
        assert_eq!(
            classify_download_source(&request("E:/tmp/file.torrent", None))?,
            DownloadSourceKind::Torrent
        );
        assert_eq!(
            classify_download_source(&request("E:/tmp/file.metalink", None))?,
            DownloadSourceKind::Metalink
        );
        assert!(classify_download_source(&request("ftp://example.com/file.zip", None)).is_err());
        assert!(classify_download_source(&request("ftps://example.com/file.zip", None)).is_err());
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn explicit_kind_overrides_source_detection() -> TestResult {
        assert_eq!(
            classify_download_source(&request(
                "https://example.com/file.torrent",
                Some(TaskKind::Http),
            ))?,
            DownloadSourceKind::Http
        );
        assert_eq!(
            classify_download_source(
                &request("https://example.com/file.zip", Some(TaskKind::Bt),)
            )?,
            DownloadSourceKind::Torrent
        );
        assert_eq!(
            classify_download_source(&request(
                "https://example.com/file.zip",
                Some(TaskKind::Metalink),
            ))?,
            DownloadSourceKind::Metalink
        );
        assert_eq!(
            classify_download_source(&request(
                "https://example.com/file.zip",
                Some(TaskKind::Sftp),
            ))?,
            DownloadSourceKind::Sftp
        );
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn routes_prefixed_ids() {
        let bt = TaskId::parse(&format!("{BT_PREFIX}1"));
        assert!(matches!(bt, TaskId::Bt(_)));
        assert_eq!(TaskId::parse("http:abc").http_inner(), "abc");
        assert_eq!(TaskId::parse("abc").http_inner(), "abc");
        assert!(!matches!(TaskId::parse("http:abc"), TaskId::Bt(_)));
    }

    #[timeout(30_000)]
    #[test]
    fn accepts_torrent_sources_for_add() {
        assert!(build_add_torrent("magnet:?xt=urn:btih:abc").is_ok());
        assert!(build_add_torrent("https://example.com/file.torrent").is_ok());
    }

    #[timeout(30_000)]
    #[test]
    fn converts_rqbit_mib_speed_to_bytes_per_second() {
        assert_eq!(mib_per_second_to_bytes_per_second(1.5), 1_572_864.0);
    }

    #[timeout(30_000)]
    #[test]
    fn upload_policy_uses_byte_limit_or_ratio_limit() {
        let settings = BtSettings {
            dht_enabled: true,
            pex_enabled: true,
            tracker_list: String::new(),
            tracker_list_url: crate::download::types::default_tracker_list_url(),
            pause_upload_when_limit_reached: true,
            upload_limit_bytes: 1024,
            upload_ratio_limit: 2.0,
        };

        assert!(upload_limit_reached(&settings, 1024, 10_000));
        assert!(upload_limit_reached(&settings, 1000, 500));
        assert!(!upload_limit_reached(&settings, 1000, 600));
    }
}
