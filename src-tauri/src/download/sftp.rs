use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    net::{TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use reqwest::Url;
use ssh2::Session;
use tokio::sync::{Mutex as AsyncMutex, broadcast};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{
    error::{DownloadError, Result},
    file_alloc, lock_or_recover,
    rate_limiter::RateLimiter,
    types::{
        AdaptiveProfile, BtUploadStatus, ChecksumMode, DownloadSnapshot, DownloadState,
        DownloadSummary, StartDownloadRequest, TaskKind, ThreadMode,
    },
};

const SFTP_PREFIX: &str = "sftp:";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const IO_TIMEOUT: Duration = Duration::from_secs(45);
const BUFFER_SIZE: usize = 128 * 1024;

pub struct SftpManager {
    state_dir: PathBuf,
    tasks: Arc<AsyncMutex<HashMap<String, Arc<SftpTask>>>>,
    event_tx: Arc<Mutex<Option<broadcast::Sender<String>>>>,
    rate_limiter: Arc<RateLimiter>,
}

struct SftpTask {
    url: String,
    destination_dir: PathBuf,
    destination_path: PathBuf,
    snapshot: Mutex<DownloadSnapshot>,
    runtime: Mutex<Option<SftpRuntime>>,
    speed: Mutex<SpeedSample>,
    rate_limiter: Arc<RateLimiter>,
}

struct SftpRuntime {
    id: Uuid,
    token: CancellationToken,
}

#[derive(Debug, Default)]
struct SpeedSample {
    bytes: u64,
    at_ms: u64,
}

struct ParsedSftpUrl {
    host: String,
    port: u16,
    username: String,
    password: Option<String>,
    remote_path: String,
}

impl SftpManager {
    pub fn new(state_dir: PathBuf, rate_limiter: Arc<RateLimiter>) -> Result<Self> {
        fs::create_dir_all(&state_dir)?;
        Ok(Self {
            state_dir,
            tasks: Arc::new(AsyncMutex::new(HashMap::new())),
            event_tx: Arc::new(Mutex::new(None)),
            rate_limiter,
        })
    }

    pub fn set_event_tx(&self, tx: broadcast::Sender<String>) {
        *lock_or_recover(&self.event_tx, "sftp event tx") = Some(tx);
    }

    pub async fn start(&self, request: StartDownloadRequest) -> Result<String> {
        let parsed = parse_sftp_url(request.url.trim())?;
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
        let file_name = request
            .file_name
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| infer_file_name(&parsed.remote_path));
        let safe_name = sanitize_filename::sanitize(file_name);
        if safe_name.is_empty() {
            return Err(DownloadError::MissingFileName);
        }

        let id = Uuid::new_v4().to_string();
        let destination_path = unique_destination_path(&destination_dir, &safe_name);
        let now = now_ms();
        let snapshot = DownloadSnapshot {
            id: sftp_task_id(&id),
            kind: TaskKind::Sftp,
            state: DownloadState::Queued,
            url: request.url.trim().to_string(),
            final_url: request.url.trim().to_string(),
            file_name: safe_name,
            destination_path: destination_path.to_string_lossy().to_string(),
            temp_path: self.state_dir.to_string_lossy().to_string(),
            total_bytes: None,
            downloaded_bytes: existing_file_len(&destination_path),
            supports_ranges: true,
            connection_count: 0,
            thread_mode: ThreadMode::Fixed,
            requested_thread_count: None,
            desired_thread_count: None,
            allocated_thread_count: None,
            adaptive_profile: None::<AdaptiveProfile>,
            thread_note: Some(String::from("SFTP transfer managed by ssh2")),
            checksum: None,
            checksum_mode: ChecksumMode::None,
            etag: None,
            last_modified: None,
            error: None,
            speed_bytes_per_second: None,
            eta_seconds: None,
            uploaded_bytes: None,
            upload_speed_bytes_per_second: None,
            peer_count: None,
            upload_status: None::<BtUploadStatus>,
            info_hash: None,
            created_at_ms: now,
            updated_at_ms: now,
            cdn_accelerated: false,
            chunks: vec![],
            seed_count: None,
            leech_count: None,
            download_limit_bps: None,
            upload_limit_bps: None,
        };

        let task = Arc::new(SftpTask {
            url: request.url.trim().to_string(),
            destination_dir,
            destination_path,
            snapshot: Mutex::new(snapshot),
            runtime: Mutex::new(None),
            speed: Mutex::new(SpeedSample {
                bytes: 0,
                at_ms: now,
            }),
            rate_limiter: self.rate_limiter.clone(),
        });

        self.tasks.lock().await.insert(id.clone(), task.clone());
        let event_tx = lock_or_recover(&self.event_tx, "sftp event tx").clone();
        let task_id = sftp_task_id(&id);
        spawn_transfer(task, event_tx, task_id.clone());
        Ok(task_id)
    }

    pub async fn pause(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let task = self.get(download_id).await?;
        cancel_runtime(&task);
        update_snapshot(&task, DownloadState::Paused, None);
        Ok(current_snapshot(&task))
    }

    pub async fn resume(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let task = self.get(download_id).await?;
        let state = lock_or_recover(&task.snapshot, "sftp snapshot").state;
        if matches!(state, DownloadState::Canceled | DownloadState::Completed) {
            return Err(DownloadError::NotResumable);
        }
        let event_tx = lock_or_recover(&self.event_tx, "sftp event tx").clone();
        spawn_transfer(task.clone(), event_tx, download_id.to_string());
        Ok(current_snapshot(&task))
    }

    pub async fn cancel(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let task = self.get(download_id).await?;
        cancel_runtime(&task);
        update_snapshot(&task, DownloadState::Canceled, None);
        Ok(current_snapshot(&task))
    }

    pub async fn remove(&self, download_id: &str) -> Result<DownloadSnapshot> {
        self.remove_task(download_id, false).await
    }

    pub async fn purge(&self, download_id: &str) -> Result<DownloadSnapshot> {
        self.remove_task(download_id, true).await
    }

    pub async fn open_in_explorer(&self, download_id: &str) -> Result<()> {
        let task = self.get(download_id).await?;
        let path = if task.destination_path.exists() {
            task.destination_path.clone()
        } else {
            task.destination_dir.clone()
        };
        if path.exists() {
            #[cfg(windows)]
            {
                std::process::Command::new("explorer").arg(path).spawn()?;
            }
            return Ok(());
        }

        Err(DownloadError::Io(io::Error::new(
            io::ErrorKind::NotFound,
            "SFTP download location does not exist",
        )))
    }

    pub async fn status(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let task = self.get(download_id).await?;
        Ok(current_snapshot(&task))
    }

    pub async fn list(&self) -> Result<Vec<DownloadSummary>> {
        let tasks = self.tasks.lock().await;
        let mut summaries = tasks
            .values()
            .map(|task| DownloadSummary::from(&current_snapshot(task)))
            .collect::<Vec<_>>();
        summaries.sort_by_key(|right| std::cmp::Reverse(right.created_at_ms));
        Ok(summaries)
    }

    async fn get(&self, download_id: &str) -> Result<Arc<SftpTask>> {
        let id = normalize_sftp_task_id(download_id);
        self.tasks
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or(DownloadError::NotFound)
    }

    async fn remove_task(&self, download_id: &str, delete_files: bool) -> Result<DownloadSnapshot> {
        let id = normalize_sftp_task_id(download_id).to_string();
        let task = self
            .tasks
            .lock()
            .await
            .remove(&id)
            .ok_or(DownloadError::NotFound)?;
        cancel_runtime(&task);
        let snapshot = current_snapshot(&task);
        if delete_files {
            remove_file_if_exists(&task.destination_path)?;
        }
        Ok(snapshot)
    }
}

pub(super) fn sftp_task_id(download_id: &str) -> String {
    format!("{SFTP_PREFIX}{download_id}")
}

fn normalize_sftp_task_id(download_id: &str) -> &str {
    download_id.strip_prefix(SFTP_PREFIX).unwrap_or(download_id)
}

fn spawn_transfer(
    task: Arc<SftpTask>,
    event_tx: Option<broadcast::Sender<String>>,
    task_id: String,
) {
    let token = CancellationToken::new();
    let runtime_id = Uuid::new_v4();
    {
        let mut runtime = lock_or_recover(&task.runtime, "sftp runtime");
        if runtime
            .as_ref()
            .is_some_and(|runtime| !runtime.token.is_cancelled())
        {
            return;
        }
        *runtime = Some(SftpRuntime {
            id: runtime_id,
            token: token.clone(),
        });
    }
    update_snapshot(&task, DownloadState::Downloading, None);

    tokio::task::spawn_blocking(move || {
        let outcome = download_sftp_file(&task, &token);
        {
            let mut runtime = lock_or_recover(&task.runtime, "sftp runtime");
            if runtime
                .as_ref()
                .is_some_and(|runtime| runtime.id == runtime_id)
            {
                *runtime = None;
            }
        }

        match outcome {
            Ok(()) => {
                update_snapshot(&task, DownloadState::Completed, None);
                if let Some(ref tx) = event_tx {
                    let gid = xxhash_rust::xxh3::xxh3_64(task_id.as_bytes());
                    let _ = tx.send(build_notification(
                        "aria2.onDownloadComplete",
                        &format!("{gid:016x}"),
                    ));
                }
            }
            Err(DownloadError::Interrupted) => {
                let state = lock_or_recover(&task.snapshot, "sftp snapshot").state;
                if !matches!(state, DownloadState::Paused | DownloadState::Canceled) {
                    update_snapshot(
                        &task,
                        DownloadState::Failed,
                        Some(String::from("interrupted")),
                    );
                    if let Some(ref tx) = event_tx {
                        let gid = xxhash_rust::xxh3::xxh3_64(task_id.as_bytes());
                        let _ = tx.send(build_notification(
                            "aria2.onDownloadError",
                            &format!("{gid:016x}"),
                        ));
                    }
                }
            }
            Err(error) => {
                update_snapshot(&task, DownloadState::Failed, Some(error.to_string()));
                if let Some(ref tx) = event_tx {
                    let gid = xxhash_rust::xxh3::xxh3_64(task_id.as_bytes());
                    let _ = tx.send(build_notification(
                        "aria2.onDownloadError",
                        &format!("{gid:016x}"),
                    ));
                }
            }
        }
    });
}

fn download_sftp_file(task: &SftpTask, token: &CancellationToken) -> Result<()> {
    let parsed = parse_sftp_url(&task.url)?;
    let address = (parsed.host.as_str(), parsed.port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| DownloadError::InvalidResponse(String::from("SFTP host did not resolve")))?;
    let tcp = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT)?;
    tcp.set_read_timeout(Some(IO_TIMEOUT))?;
    tcp.set_write_timeout(Some(IO_TIMEOUT))?;

    let mut session = Session::new().map_err(|error| DownloadError::Sftp(error.to_string()))?;
    session.set_tcp_stream(tcp);
    session
        .handshake()
        .map_err(|error| DownloadError::Sftp(error.to_string()))?;
    authenticate(&mut session, &parsed)?;

    let sftp = session
        .sftp()
        .map_err(|error| DownloadError::Sftp(error.to_string()))?;
    let mut remote = sftp
        .open(Path::new(&parsed.remote_path))
        .map_err(|error| DownloadError::Sftp(error.to_string()))?;
    let total_bytes = remote
        .stat()
        .ok()
        .and_then(|stat| stat.size)
        .filter(|size| *size > 0);
    if let Some(total_bytes) = total_bytes {
        let mut snapshot = lock_or_recover(&task.snapshot, "sftp snapshot");
        snapshot.total_bytes = Some(total_bytes);
    }

    // Compute how much of the file already exists locally (for resume support).
    let already_downloaded = existing_file_len(&task.destination_path);

    // Check available disk space before starting the transfer.
    // Subtract already-downloaded bytes so a resumed download isn't falsely rejected.
    if let Some(total_bytes) = total_bytes {
        let needed = total_bytes.saturating_sub(already_downloaded);
        file_alloc::check_disk_space(&task.destination_dir, needed)?;
    }

    let offset = total_bytes
        .map(|total| already_downloaded.min(total))
        .unwrap_or(already_downloaded);
    remote
        .seek(SeekFrom::Start(offset))
        .map_err(|error| DownloadError::Sftp(error.to_string()))?;

    if let Some(parent) = task.destination_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut local = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(offset == 0)
        .open(&task.destination_path)?;
    local.seek(SeekFrom::Start(offset))?;

    let mut buffer = vec![0_u8; BUFFER_SIZE];
    loop {
        if token.is_cancelled() {
            return Err(DownloadError::Interrupted);
        }

        let read = remote
            .read(&mut buffer)
            .map_err(|error| DownloadError::Sftp(error.to_string()))?;
        if read == 0 {
            break;
        }
        task.rate_limiter.consume_blocking(read);
        local.write_all(&buffer[..read])?;
        refresh_progress(task);
    }
    local.flush()?;
    Ok(())
}

fn authenticate(session: &mut Session, parsed: &ParsedSftpUrl) -> Result<()> {
    if let Some(password) = parsed.password.as_deref() {
        session
            .userauth_password(&parsed.username, password)
            .map_err(|error| DownloadError::Sftp(error.to_string()))?;
    } else {
        let mut agent = session
            .agent()
            .map_err(|error| DownloadError::Sftp(error.to_string()))?;
        agent
            .connect()
            .map_err(|error| DownloadError::Sftp(error.to_string()))?;
        agent
            .list_identities()
            .map_err(|error| DownloadError::Sftp(error.to_string()))?;
        let mut authenticated = false;
        for identity in agent
            .identities()
            .map_err(|error| DownloadError::Sftp(error.to_string()))?
        {
            if agent.userauth(&parsed.username, &identity).is_ok() {
                authenticated = true;
                break;
            }
        }
        if !authenticated {
            return Err(DownloadError::Sftp(String::from(
                "SFTP authentication failed; include a password in the URL or load an SSH agent identity",
            )));
        }
    }

    if !session.authenticated() {
        return Err(DownloadError::Sftp(String::from(
            "SFTP authentication failed",
        )));
    }
    Ok(())
}

fn parse_sftp_url(source: &str) -> Result<ParsedSftpUrl> {
    let url =
        Url::parse(source).map_err(|error| DownloadError::InvalidResponse(error.to_string()))?;
    if url.scheme() != "sftp" {
        return Err(DownloadError::UnsupportedScheme);
    }
    let host = url
        .host_str()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| DownloadError::InvalidResponse(String::from("SFTP URL is missing a host")))?
        .to_string();
    let username = percent_decode(url.username());
    if username.trim().is_empty() {
        return Err(DownloadError::InvalidResponse(String::from(
            "SFTP URL is missing a username",
        )));
    }
    let remote_path = percent_decode(url.path());
    if remote_path.trim_matches('/').is_empty() {
        return Err(DownloadError::MissingFileName);
    }

    Ok(ParsedSftpUrl {
        host,
        port: url.port().unwrap_or(22),
        username,
        password: url.password().map(percent_decode),
        remote_path,
    })
}

fn infer_file_name(remote_path: &str) -> String {
    Path::new(remote_path)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| String::from("download"))
}

fn unique_destination_path(destination_dir: &Path, file_name: &str) -> PathBuf {
    let base = destination_dir.join(file_name);
    if !base.exists() {
        return base;
    }

    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("download");
    let extension = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{value}"))
        .unwrap_or_default();

    for index in 1..10_000 {
        let candidate = destination_dir.join(format!("{stem} ({index}){extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }

    destination_dir.join(format!("{}-{}{}", stem, Uuid::new_v4(), extension))
}

fn cancel_runtime(task: &SftpTask) {
    let mut runtime = lock_or_recover(&task.runtime, "sftp runtime");
    if let Some(runtime) = runtime.take() {
        runtime.token.cancel();
    }
}

fn current_snapshot(task: &SftpTask) -> DownloadSnapshot {
    refresh_progress(task);
    lock_or_recover(&task.snapshot, "sftp snapshot").clone()
}

fn refresh_progress(task: &SftpTask) {
    let now = now_ms();
    let downloaded = existing_file_len(&task.destination_path);
    let speed = {
        let mut speed = lock_or_recover(&task.speed, "sftp speed");
        let elapsed = now.saturating_sub(speed.at_ms);
        let next = if elapsed > 0 && downloaded >= speed.bytes {
            Some((downloaded - speed.bytes) as f64 * 1000.0 / elapsed as f64)
        } else {
            None
        };
        speed.bytes = downloaded;
        speed.at_ms = now;
        next
    };

    let mut snapshot = lock_or_recover(&task.snapshot, "sftp snapshot");
    snapshot.downloaded_bytes = downloaded;
    let terminal = matches!(
        snapshot.state,
        DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
    );
    snapshot.speed_bytes_per_second = if terminal {
        None
    } else {
        speed.filter(|value| *value > 0.0)
    };
    snapshot.eta_seconds = if terminal {
        None
    } else {
        estimate_eta(
            snapshot.total_bytes,
            downloaded,
            snapshot.speed_bytes_per_second,
        )
    };
    snapshot.updated_at_ms = now;
}

fn update_snapshot(task: &SftpTask, state: DownloadState, error: Option<String>) {
    let mut snapshot = lock_or_recover(&task.snapshot, "sftp snapshot");
    snapshot.state = state;
    snapshot.downloaded_bytes = existing_file_len(&task.destination_path);
    snapshot.connection_count = usize::from(matches!(state, DownloadState::Downloading));
    snapshot.error = error;
    snapshot.updated_at_ms = now_ms();
}

fn estimate_eta(total: Option<u64>, downloaded: u64, speed: Option<f64>) -> Option<u64> {
    let total = total?;
    let speed = speed?;
    if total <= downloaded || speed <= 0.0 {
        return None;
    }
    Some(((total - downloaded) as f64 / speed).ceil() as u64)
}

fn existing_file_len(path: &Path) -> u64 {
    fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0)
}

fn remove_file_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn percent_decode(value: &str) -> String {
    urlencoding::decode(value)
        .map(|decoded| decoded.into_owned())
        .unwrap_or_else(|_| value.to_string())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn build_notification(method: &str, gid: &str) -> String {
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

    use super::{infer_file_name, parse_sftp_url};

    type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

    #[timeout(30_000)]
    #[test]
    fn parses_sftp_url() -> TestResult {
        let parsed = parse_sftp_url("sftp://alice:secret@example.com:2222/path/file.zip")?;

        assert_eq!(parsed.host, "example.com");
        assert_eq!(parsed.port, 2222);
        assert_eq!(parsed.username, "alice");
        assert_eq!(parsed.password.as_deref(), Some("secret"));
        assert_eq!(parsed.remote_path, "/path/file.zip");
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn rejects_non_sftp_url() {
        assert!(parse_sftp_url("ftp://example.com/file.zip").is_err());
        assert!(parse_sftp_url("ftps://example.com/file.zip").is_err());
    }

    #[timeout(30_000)]
    #[test]
    fn infers_remote_file_name() {
        assert_eq!(
            infer_file_name("/var/data/archive.tar.gz"),
            "archive.tar.gz"
        );
    }
}
