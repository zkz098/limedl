use std::path::PathBuf;

pub fn dirs_or_temp_dir() -> PathBuf {
    // Explicit override (same env var the headless server honors) — makes it
    // possible to run a throwaway instance against a temp data dir.
    if let Some(dir) = std::env::var_os("LIMEDL_DATA_DIR") {
        return PathBuf::from(dir);
    }
    if let Some(dir) = dirs_local_data_dir() {
        dir.join("limedl")
    } else {
        std::env::temp_dir().join("limedl")
    }
}

pub fn dirs_local_data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    }
}

pub fn dirs_download_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .map(|p| p.join("Downloads"))
            .filter(|p| p.exists())
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|p| p.join("Downloads"))
            .filter(|p| p.exists())
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        std::env::var_os("XDG_DOWNLOAD_DIR")
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .map(|h| h.join("Downloads"))
                    .filter(|p| p.exists())
            })
    }
}
