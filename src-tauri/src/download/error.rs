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
}

pub type Result<T> = std::result::Result<T, DownloadError>;
