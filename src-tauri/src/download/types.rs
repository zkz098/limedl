use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChecksumMode {
    None,
    Blake3,
}

impl Default for ChecksumMode {
    fn default() -> Self {
        Self::Blake3
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadState {
    Queued,
    Downloading,
    Paused,
    Retrying,
    Verifying,
    Completed,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartDownloadRequest {
    pub url: String,
    pub destination_dir: String,
    pub file_name: Option<String>,
    pub max_connections: Option<usize>,
    pub max_retries: Option<u32>,
    pub checksum: Option<ChecksumMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadSnapshot {
    pub id: String,
    pub state: DownloadState,
    pub url: String,
    pub final_url: String,
    pub file_name: String,
    pub destination_path: String,
    pub temp_path: String,
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub supports_ranges: bool,
    pub connection_count: usize,
    pub checksum: Option<String>,
    pub checksum_mode: ChecksumMode,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub error: Option<String>,
    pub speed_bytes_per_second: Option<f64>,
    pub eta_seconds: Option<u64>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadSummary {
    pub id: String,
    pub state: DownloadState,
    pub file_name: String,
    pub destination_path: String,
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub connection_count: usize,
    pub speed_bytes_per_second: Option<f64>,
    pub eta_seconds: Option<u64>,
    pub error: Option<String>,
}

impl From<&DownloadSnapshot> for DownloadSummary {
    fn from(value: &DownloadSnapshot) -> Self {
        Self {
            id: value.id.clone(),
            state: value.state,
            file_name: value.file_name.clone(),
            destination_path: value.destination_path.clone(),
            total_bytes: value.total_bytes,
            downloaded_bytes: value.downloaded_bytes,
            connection_count: value.connection_count,
            speed_bytes_per_second: value.speed_bytes_per_second,
            eta_seconds: value.eta_seconds,
            error: value.error.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProxyMode {
    Disabled,
    System,
    Manual,
}

impl Default for ProxyMode {
    fn default() -> Self {
        Self::Disabled
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxySettings {
    pub mode: ProxyMode,
    pub manual_url: String,
}
