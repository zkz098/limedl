use std::{
    collections::HashMap,
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

use super::{
    error::{DownloadError, Result},
    types::{
        AppSettings, BtSettings, BtUploadStatus, ChecksumMode, DownloadSnapshot, DownloadState,
        DownloadSummary, ProxyMode, StartDownloadRequest, TaskKind, ThreadMode,
    },
};

pub(super) const HTTP_PREFIX: &str = "http:";
pub(super) const BT_PREFIX: &str = "bt:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DownloadSourceKind {
    Http,
    Torrent,
}

pub struct TorrentManager {
    session: Arc<Session>,
    api: Api,
    state_dir: PathBuf,
    default_output_dir: PathBuf,
    bt_settings: Arc<Mutex<BtSettings>>,
    output_folders: Arc<Mutex<HashMap<usize, PathBuf>>>,
}

impl TorrentManager {
    pub async fn new(state_dir: PathBuf, settings: &AppSettings) -> Result<Self> {
        fs::create_dir_all(&state_dir)?;
        let output_dir = state_dir.join("files");
        let persistence_dir = state_dir.join("session");
        fs::create_dir_all(&output_dir)?;
        fs::create_dir_all(&persistence_dir)?;

        let mut options = SessionOptions {
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

        let session = Session::new_with_opts(output_dir.clone(), options)
            .await
            .map_err(|error| DownloadError::Torrent(error.to_string()))?;

        let api = Api::new(session.clone(), None);

        Ok(Self {
            session,
            api,
            state_dir: state_dir.clone(),
            default_output_dir: output_dir,
            bt_settings: Arc::new(Mutex::new(settings.bt.clone())),
            output_folders: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn update_settings(&self, settings: &AppSettings) {
        *self.bt_settings.lock().expect("bt settings poisoned") = settings.bt.clone();
    }

    pub async fn start(&self, request: StartDownloadRequest) -> Result<String> {
        let source = request.url.trim();
        if source.is_empty() {
            return Err(DownloadError::InvalidResponse(String::from(
                "torrent source is empty",
            )));
        }

        let destination_dir = PathBuf::from(request.destination_dir.trim());
        fs::create_dir_all(&destination_dir)?;

        let add = build_add_torrent(source)?;
        let options = AddTorrentOptions {
            output_folder: Some(destination_dir.to_string_lossy().to_string()),
            overwrite: true,
            ..AddTorrentOptions::default()
        };

        let id = match self
            .session
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

        self.output_folders
            .lock()
            .expect("output folders poisoned")
            .insert(id, destination_dir);

        Ok(bt_task_id(id))
    }

    pub async fn pause(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let id = parse_bt_task_id(download_id)?;
        let handle = self.get_handle(id)?;
        self.session
            .pause(&handle)
            .await
            .map_err(|error| DownloadError::Torrent(error.to_string()))?;
        self.status(download_id).await
    }

    pub async fn resume(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let id = parse_bt_task_id(download_id)?;
        let handle = self.get_handle(id)?;
        self.session
            .unpause(&handle)
            .await
            .map_err(|error| DownloadError::Torrent(error.to_string()))?;
        self.status(download_id).await
    }

    pub async fn cancel(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let snapshot = self.status(download_id).await?;
        self.delete(download_id, false).await?;
        Ok(DownloadSnapshot {
            state: DownloadState::Canceled,
            updated_at_ms: now_ms(),
            ..snapshot
        })
    }

    pub async fn remove(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let snapshot = self.status(download_id).await?;
        self.delete(download_id, false).await?;
        Ok(snapshot)
    }

    pub async fn purge(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let snapshot = self.status(download_id).await?;
        self.delete(download_id, true).await?;
        Ok(snapshot)
    }

    pub async fn open_in_explorer(&self, download_id: &str) -> Result<()> {
        let snapshot = self.status(download_id).await?;
        let path = PathBuf::from(&snapshot.destination_path);
        if path.exists() {
            Command::new("explorer").arg(&path).spawn()?;
            return Ok(());
        }

        Err(DownloadError::Io(io::Error::new(
            io::ErrorKind::NotFound,
            "torrent download location does not exist",
        )))
    }

    pub async fn status(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let id = parse_bt_task_id(download_id)?;
        let handle = self.get_handle(id)?;
        let limit_reached = self.apply_upload_policy(&handle).await?;
        Ok(self.snapshot_from_handle(id, &handle, limit_reached))
    }

    pub async fn list(&self) -> Result<Vec<DownloadSummary>> {
        let handles = self.session.with_torrents(|torrents| {
            torrents
                .map(|(id, handle)| (id, handle.clone()))
                .collect::<Vec<_>>()
        });
        let mut summaries = Vec::with_capacity(handles.len());
        for (id, handle) in handles {
            let limit_reached = self.apply_upload_policy(&handle).await?;
            summaries.push(DownloadSummary::from(&self.snapshot_from_handle(
                id,
                &handle,
                limit_reached,
            )));
        }
        summaries.sort_by(|left, right| right.id.cmp(&left.id));
        Ok(summaries)
    }

    fn get_handle(&self, id: usize) -> Result<Arc<ManagedTorrent>> {
        self.session
            .get(TorrentIdOrHash::Id(id))
            .ok_or(DownloadError::NotFound)
    }

    async fn delete(&self, download_id: &str, delete_files: bool) -> Result<()> {
        let id = parse_bt_task_id(download_id)?;
        self.session
            .delete(TorrentIdOrHash::Id(id), delete_files)
            .await
            .map_err(|error| DownloadError::Torrent(error.to_string()))?;
        self.output_folders
            .lock()
            .expect("output folders poisoned")
            .remove(&id);
        Ok(())
    }

    async fn apply_upload_policy(&self, handle: &Arc<ManagedTorrent>) -> Result<bool> {
        let stats = handle.stats();
        let settings = self
            .bt_settings
            .lock()
            .expect("bt settings poisoned")
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
                    .expect("output folders poisoned")
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
            speed_bytes_per_second: speed,
            eta_seconds: estimate_eta(stats.total_bytes, downloaded, speed),
            uploaded_bytes: Some(stats.uploaded_bytes),
            peer_count: Some(peer_count),
            upload_status: Some(upload_status),
            created_at_ms: now,
            updated_at_ms: now,
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
        });
    }

    let source = request.url.trim();
    let lower = source.to_ascii_lowercase();

    if lower.starts_with("magnet:") || lower.ends_with(".torrent") {
        return Ok(DownloadSourceKind::Torrent);
    }

    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Ok(DownloadSourceKind::Http);
    }

    let path = Path::new(source);
    if path.extension().and_then(|value| value.to_str()) == Some("torrent") {
        return Ok(DownloadSourceKind::Torrent);
    }

    Err(DownloadError::UnsupportedScheme)
}

pub(super) fn normalize_http_task_id(download_id: &str) -> &str {
    download_id.strip_prefix(HTTP_PREFIX).unwrap_or(download_id)
}

pub(super) fn is_bt_task_id(download_id: &str) -> bool {
    download_id.starts_with(BT_PREFIX)
}

pub(super) fn http_task_id(download_id: String) -> String {
    format!("{HTTP_PREFIX}{download_id}")
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

#[cfg(test)]
mod tests {
    use super::{
        BT_PREFIX, DownloadSourceKind, build_add_torrent, classify_download_source, is_bt_task_id,
        mib_per_second_to_bytes_per_second, normalize_http_task_id, upload_limit_reached,
    };
    use crate::download::types::{BtSettings, StartDownloadRequest, TaskKind};

    fn request(url: &str, kind: Option<TaskKind>) -> StartDownloadRequest {
        StartDownloadRequest {
            kind,
            url: String::from(url),
            destination_dir: String::from("E:/tmp"),
            file_name: None,
            thread_mode: None,
            thread_count: None,
            max_retries: None,
            checksum: None,
        }
    }

    #[test]
    fn classifies_download_sources() {
        assert_eq!(
            classify_download_source(&request("https://example.com/file.zip", None)).unwrap(),
            DownloadSourceKind::Http
        );
        assert_eq!(
            classify_download_source(&request("magnet:?xt=urn:btih:abc", None)).unwrap(),
            DownloadSourceKind::Torrent
        );
        assert_eq!(
            classify_download_source(&request("https://example.com/file.torrent", None)).unwrap(),
            DownloadSourceKind::Torrent
        );
        assert_eq!(
            classify_download_source(&request("E:/tmp/file.torrent", None)).unwrap(),
            DownloadSourceKind::Torrent
        );
        assert!(classify_download_source(&request("ftp://example.com/file.zip", None)).is_err());
    }

    #[test]
    fn explicit_kind_overrides_source_detection() {
        assert_eq!(
            classify_download_source(&request(
                "https://example.com/file.torrent",
                Some(TaskKind::Http),
            ))
            .unwrap(),
            DownloadSourceKind::Http
        );
        assert_eq!(
            classify_download_source(&request("https://example.com/file.zip", Some(TaskKind::Bt),))
                .unwrap(),
            DownloadSourceKind::Torrent
        );
    }

    #[test]
    fn routes_prefixed_ids() {
        assert!(is_bt_task_id(&format!("{BT_PREFIX}1")));
        assert_eq!(normalize_http_task_id("http:abc"), "abc");
        assert_eq!(normalize_http_task_id("abc"), "abc");
    }

    #[test]
    fn accepts_torrent_sources_for_add() {
        assert!(build_add_torrent("magnet:?xt=urn:btih:abc").is_ok());
        assert!(build_add_torrent("https://example.com/file.torrent").is_ok());
    }

    #[test]
    fn converts_rqbit_mib_speed_to_bytes_per_second() {
        assert_eq!(mib_per_second_to_bytes_per_second(1.5), 1_572_864.0);
    }

    #[test]
    fn upload_policy_uses_byte_limit_or_ratio_limit() {
        let settings = BtSettings {
            pause_upload_when_limit_reached: true,
            upload_limit_bytes: 1024,
            upload_ratio_limit: 2.0,
        };

        assert!(upload_limit_reached(&settings, 1024, 10_000));
        assert!(upload_limit_reached(&settings, 1000, 500));
        assert!(!upload_limit_reached(&settings, 1000, 600));
    }
}
