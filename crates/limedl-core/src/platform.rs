use std::io;
use std::path::Path;

/// Open a path in the system file manager.
/// On Windows, uses `explorer` to open the directory.
/// On macOS, uses `open`.
/// On Linux, uses `xdg-open`.
pub fn open_in_file_manager(path: &Path) -> io::Result<()> {
    if !path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("path does not exist: {}", path.display()),
        ));
    }
    #[cfg(windows)]
    {
        std::process::Command::new("explorer").arg(path).spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(path).spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        let output = std::process::Command::new("xdg-open")
            .arg(path)
            .output()
            .map_err(|e| {
                io::Error::new(e.kind(), format!("failed to launch xdg-open for {}: {e}", path.display()))
            })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(io::Error::other(
                format!(
                    "xdg-open exited with {} for {}: {}",
                    output.status,
                    path.display(),
                    if stderr.is_empty() { "no desktop session or file manager available".to_string() } else { stderr }
                ),
            ));
        }
    }
    Ok(())
}

/// Reveal a file in the system file manager (selecting it when supported).
/// On Windows, uses `explorer /select,`; on macOS, `open -R`.
/// On Linux, falls back to opening the containing directory via `xdg-open`
/// (xdg-open cannot select individual files).
pub fn reveal_in_file_manager(path: &Path) -> io::Result<()> {
    if !path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("path does not exist: {}", path.display()),
        ));
    }
    #[cfg(windows)]
    {
        std::process::Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg("-R").arg(path).spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(path);
        open_in_file_manager(dir)?;
    }
    Ok(())
}
