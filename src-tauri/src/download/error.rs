use thiserror::Error;

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("unsupported url scheme")]
    UnsupportedScheme,
    #[error("download not found")]
    NotFound,
    #[error("download is already running")]
    AlreadyRunning,
    #[error("download is not paused or failed")]
    NotResumable,
    #[error("download was canceled and cannot be resumed")]
    Canceled,
    #[error("server does not provide a file name and no fallback could be derived")]
    MissingFileName,
    #[error("download interrupted")]
    Interrupted,
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("invalid server response: {0}")]
    InvalidResponse(String),
    #[error("invalid proxy configuration: {0}")]
    InvalidProxy(String),
    #[error("torrent error: {0}")]
    Torrent(String),
    #[error("sftp error: {0}")]
    Sftp(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl DownloadError {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::UnsupportedScheme => "unsupported_scheme",
            Self::NotFound => "not_found",
            Self::AlreadyRunning => "already_running",
            Self::NotResumable => "not_resumable",
            Self::Canceled => "canceled",
            Self::MissingFileName => "missing_file_name",
            Self::Interrupted => "interrupted",
            Self::Http(_) => "http",
            Self::Io(_) => "io",
            Self::Serde(_) => "serde",
            Self::InvalidResponse(_) => "invalid_response",
            Self::InvalidProxy(_) => "invalid_proxy",
            Self::Torrent(_) => "torrent",
            Self::Sftp(_) => "sftp",
            Self::Internal(_) => "internal",
        }
    }
}

impl From<anyhow::Error> for DownloadError {
    fn from(error: anyhow::Error) -> Self {
        Self::Internal(format!("{error:#}"))
    }
}

pub type Result<T> = std::result::Result<T, DownloadError>;
