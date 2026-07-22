use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
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
    #[error("permission denied: {path}")]
    PermissionDenied {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("invalid server response: {0}")]
    InvalidResponse(String),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("invalid proxy configuration: {0}")]
    InvalidProxy(String),
    #[error("torrent error: {0}")]
    Torrent(String),
    #[error("torrent network error: {0}")]
    TorrentNetwork(String),
    #[error("torrent data error: {0}")]
    TorrentInvalidData(String),
    #[error("torrent io error: {0}")]
    TorrentIo(String),
    #[error(
        "insufficient disk space: {available} bytes available, {required} bytes required (incl. 10% buffer)"
    )]
    InsufficientDiskSpace { available: u64, required: u64 },
    #[error("database initialization error: {0}")]
    DatabaseInit(String),
    #[error("internal error: {0}")]
    Internal(String),
    #[error("too many concurrent downloads")]
    TooManyConcurrentDownloads,
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
            Self::PermissionDenied { .. } => "permission_denied",
            Self::Serde(_) => "serde",
            Self::InvalidResponse(_) => "invalid_response",
            Self::InvalidRequest(_) => "invalid_request",
            Self::InvalidProxy(_) => "invalid_proxy",
            Self::Torrent(_) => "torrent",
            Self::TorrentNetwork(_) => "torrent_network",
            Self::TorrentInvalidData(_) => "torrent_invalid_data",
            Self::TorrentIo(_) => "torrent_io",
            Self::InsufficientDiskSpace { .. } => "insufficient_disk_space",
            Self::DatabaseInit(_) => "database_init",
            Self::TooManyConcurrentDownloads => "too_many_concurrent_downloads",
            Self::Internal(_) => "internal",
            #[allow(unreachable_patterns)]
            _ => "unknown",
        }
    }
}

/// Convert an `std::io::Error` to `DownloadError`, distinguishing PermissionDenied.
/// Use at file I/O call sites to produce a user-friendly error with path context.
pub fn io_error_with_path(error: std::io::Error, path: impl Into<String>) -> DownloadError {
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        DownloadError::PermissionDenied {
            path: path.into(),
            source: error,
        }
    } else {
        DownloadError::Io(error)
    }
}

/// Walk the anyhow error chain to find the original `DownloadError` kind.
/// Returns `"internal"` if no `DownloadError` is found in the chain.
pub fn extract_kind_from_anyhow(error: &anyhow::Error) -> &'static str {
    for cause in error.chain() {
        if let Some(dl_err) = cause.downcast_ref::<DownloadError>() {
            return dl_err.kind();
        }
    }
    "internal"
}

impl From<anyhow::Error> for DownloadError {
    fn from(error: anyhow::Error) -> Self {
        // If the error IS a DownloadError (no context added), extract it directly.
        // `downcast` returns Err(error) on mismatch, giving it back to us.
        match error.downcast::<DownloadError>() {
            Ok(dl_err) => dl_err,
            Err(error) => {
                // Walk the chain to find a DownloadError and preserve its kind in the message.
                let kind = extract_kind_from_anyhow(&error);
                if kind == "internal" {
                    Self::Internal(format!("{error:#}"))
                } else {
                    Self::Internal(format!("[{kind}] {error:#}"))
                }
            }
        }
    }
}


pub type Result<T> = std::result::Result<T, DownloadError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_error_with_path_permission_denied() {
        let err = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        let download_err = io_error_with_path(err, "/test/path");
        assert_eq!(download_err.kind(), "permission_denied");
    }

    #[test]
    fn io_error_with_path_other_io_error() {
        let err = std::io::Error::from_raw_os_error(2); // ERROR_FILE_NOT_FOUND
        let download_err = io_error_with_path(err, "/test/path");
        assert_eq!(download_err.kind(), "io");
    }

    #[test]
    fn extract_kind_from_anyhow_direct() {
        let err = anyhow::Error::from(DownloadError::NotFound);
        let kind = extract_kind_from_anyhow(&err);
        assert_eq!(kind, "not_found");
    }

    #[test]
    fn from_anyhow_direct_download_error() {
        let anyhow_err = anyhow::Error::from(DownloadError::NotFound);
        let result: DownloadError = anyhow_err.into();
        assert_eq!(result.kind(), "not_found");
    }

    #[test]
    fn from_anyhow_internal_with_kind_prefix() {
        // An anyhow error with [kind] prefix in message but no DownloadError in chain
        let anyhow_err = anyhow::anyhow!("[permission_denied] failed to write");
        let result: DownloadError = anyhow_err.into();
        // Falls through to Internal because the [kind] prefix is just text, not parsed
        assert_eq!(result.kind(), "internal");
    }

    #[test]
    fn from_anyhow_bare_internal_error() {
        let anyhow_err = anyhow::anyhow!("something went wrong");
        let result: DownloadError = anyhow_err.into();
        assert_eq!(result.kind(), "internal");
    }

    /// Exhaustively tests that every `DownloadError` variant returns the correct `kind()` string.
    /// This catches missing `kind()` arms when new variants are added — the `_ => "unknown"`
    /// fallback silences the compiler, so this test provides the safety net.
    #[test]
    fn download_error_kind_exhaustive() {
        // ---- Variants without payload ----
        assert_eq!(DownloadError::UnsupportedScheme.kind(), "unsupported_scheme");
        assert_eq!(DownloadError::NotFound.kind(), "not_found");
        assert_eq!(DownloadError::AlreadyRunning.kind(), "already_running");
        assert_eq!(DownloadError::NotResumable.kind(), "not_resumable");
        assert_eq!(DownloadError::Canceled.kind(), "canceled");
        assert_eq!(DownloadError::MissingFileName.kind(), "missing_file_name");
        assert_eq!(DownloadError::Interrupted.kind(), "interrupted");
        assert_eq!(DownloadError::TooManyConcurrentDownloads.kind(), "too_many_concurrent_downloads");

        // ---- Variants with String payload ----
        assert_eq!(DownloadError::InvalidResponse("test".into()).kind(), "invalid_response");
        assert_eq!(DownloadError::InvalidRequest("test".into()).kind(), "invalid_request");
        assert_eq!(DownloadError::InvalidProxy("test".into()).kind(), "invalid_proxy");
        assert_eq!(DownloadError::Torrent("test".into()).kind(), "torrent");
        assert_eq!(DownloadError::TorrentNetwork("test".into()).kind(), "torrent_network");
        assert_eq!(DownloadError::TorrentInvalidData("test".into()).kind(), "torrent_invalid_data");
        assert_eq!(DownloadError::TorrentIo("test".into()).kind(), "torrent_io");
        assert_eq!(DownloadError::DatabaseInit("test".into()).kind(), "database_init");
        assert_eq!(DownloadError::Internal("test".into()).kind(), "internal");

        // ---- Variants wrapping external error types ----
        assert_eq!(
            DownloadError::Io(std::io::Error::other("test")).kind(),
            "io",
        );
        let serde_err = serde_json::from_str::<()>("invalid json").unwrap_err();
        assert_eq!(DownloadError::Serde(serde_err).kind(), "serde");

        // Construct a reqwest::Error via build() with an invalid URL
        let http_err = reqwest::Client::new()
            .get("http://") // empty host triggers URL parse error
            .build()
            .unwrap_err();
        assert_eq!(DownloadError::Http(http_err).kind(), "http");

        // ---- Struct variants ----
        assert_eq!(
            DownloadError::PermissionDenied {
                path: "/data/file".into(),
                source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied"),
            }
            .kind(),
            "permission_denied",
        );
        assert_eq!(
            DownloadError::InsufficientDiskSpace {
                available: 0,
                required: 100,
            }
            .kind(),
            "insufficient_disk_space",
        );
    }
}
