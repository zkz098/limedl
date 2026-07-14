use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, BufReader, ErrorKind, Read, Write},
    path::Path,
};

use fs4::fs_std::FileExt;

use super::error::{DownloadError, Result};

pub(super) fn open_download_file(path: &Path, total_size: Option<u64>) -> Result<File> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;

    preallocate_file(&file, total_size)?;
    Ok(file)
}

pub(super) fn reset_download_file(file: &File, total_size: Option<u64>) -> Result<()> {
    file.set_len(0)?;
    preallocate_file(file, total_size)
}

pub(super) fn finalize_temp_file(temp_path: &Path, destination_path: &Path) -> Result<()> {
    if destination_path.exists() {
        if files_have_same_content(temp_path, destination_path)? {
            cleanup_finalizing_paths(destination_path)?;
            fs::remove_file(temp_path)?;
            return Ok(());
        }
        return Err(destination_exists_error(destination_path));
    }

    // Primary path: atomic rename (works on the same filesystem).
    // rename is a metadata-only operation — no data copy, no sync_all needed.
    // On same-volume moves this is O(1) and atomic on both POSIX and Windows.
    match std::fs::rename(temp_path, destination_path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::CrossesDevices => {
            // Fallback: copy via staging path when source and destination
            // reside on different mount points / drive letters.
            let staging_path = unique_finalizing_path(destination_path)?;
            let mut source = File::open(temp_path)?;
            let mut destination = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&staging_path)
                .map_err(DownloadError::Io)?;

            if let Err(error) = io::copy(&mut source, &mut destination) {
                drop(destination);
                drop(source);
                let _ = fs::remove_file(&staging_path);
                return Err(error.into());
            }
            destination.flush()?;
            destination.sync_all()?;
            drop(destination);
            drop(source);

            if let Err(error) = fs::hard_link(&staging_path, destination_path) {
                if error.kind() == ErrorKind::AlreadyExists
                    && files_have_same_content(&staging_path, destination_path)?
                {
                    fs::remove_file(&staging_path)?;
                    fs::remove_file(temp_path)?;
                    return Ok(());
                }
                if error.kind() == ErrorKind::AlreadyExists {
                    let _ = fs::remove_file(&staging_path);
                    return Err(destination_exists_error(destination_path));
                }

                fallback_copy_staging_to_destination(&staging_path, destination_path)?;
            }

            fs::remove_file(&staging_path)?;
            fs::remove_file(temp_path)?;
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

fn fallback_copy_staging_to_destination(
    staging_path: &Path,
    destination_path: &Path,
) -> Result<()> {
    let mut source = File::open(staging_path)?;
    let mut destination = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination_path)
        .map_err(|error| {
            if error.kind() == ErrorKind::AlreadyExists {
                destination_exists_error(destination_path)
            } else {
                DownloadError::Io(error)
            }
        })?;

    if let Err(error) = io::copy(&mut source, &mut destination) {
        drop(destination);
        drop(source);
        let _ = fs::remove_file(destination_path);
        return Err(error.into());
    }
    destination.flush()?;
    destination.sync_all()?;
    Ok(())
}

fn destination_exists_error(destination_path: &Path) -> DownloadError {
    DownloadError::Io(io::Error::new(
        ErrorKind::AlreadyExists,
        format!(
            "destination file already exists: {}",
            destination_path.display()
        ),
    ))
}

fn files_have_same_content(left_path: &Path, right_path: &Path) -> Result<bool> {
    let left = File::open(left_path)?;
    let right = File::open(right_path)?;
    if left.metadata()?.len() != right.metadata()?.len() {
        return Ok(false);
    }

    let mut left = BufReader::new(left);
    let mut right = BufReader::new(right);
    let mut left_buffer = [0; 8192];
    let mut right_buffer = [0; 8192];
    loop {
        let left_read = left.read(&mut left_buffer)?;
        let right_read = right.read(&mut right_buffer)?;
        if left_read != right_read {
            return Ok(false);
        }
        if left_read == 0 {
            return Ok(true);
        }
        if left_buffer[..left_read] != right_buffer[..right_read] {
            return Ok(false);
        }
    }
}

fn cleanup_finalizing_paths(destination_path: &Path) -> Result<()> {
    let Some(parent) = destination_path.parent() else {
        return Ok(());
    };
    let Some(file_name) = destination_path.file_name() else {
        return Ok(());
    };
    let prefix = {
        let mut value = OsString::from(file_name);
        value.push(".finalizing.");
        value
    };

    for entry in fs::read_dir(parent)? {
        let entry = entry?;
        let candidate_name = entry.file_name();
        if candidate_name
            .to_string_lossy()
            .starts_with(&prefix.to_string_lossy().to_string())
        {
            let _ = fs::remove_file(entry.path());
        }
    }
    Ok(())
}

fn unique_finalizing_path(destination_path: &Path) -> Result<std::path::PathBuf> {
    let parent = destination_path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = destination_path.file_name().ok_or_else(|| {
        DownloadError::InvalidResponse(String::from("missing destination file name"))
    })?;
    let process_id = std::process::id();

    for attempt in 0..1000u16 {
        let mut staging_name = OsString::from(file_name);
        staging_name.push(format!(".finalizing.{process_id}.{attempt}.tmp"));
        let candidate = parent.join(staging_name);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(DownloadError::Io(io::Error::new(
        ErrorKind::AlreadyExists,
        format!(
            "unable to allocate finalizing path for {}",
            destination_path.display()
        ),
    )))
}

pub(super) fn write_all_at(file: &File, mut buffer: &[u8], mut offset: u64) -> Result<()> {
    while !buffer.is_empty() {
        let written = write_once_at(file, buffer, offset)?;
        if written == 0 {
            return Err(DownloadError::InvalidResponse(String::from(
                "failed to write download data",
            )));
        }
        offset += written as u64;
        buffer = &buffer[written..];
    }
    Ok(())
}

/// Checks that the destination directory has enough free space for the download.
/// `required_bytes` is the total file size. We require 10% buffer above that.
pub(super) fn check_disk_space(destination_dir: &Path, required_bytes: u64) -> Result<()> {
    let available = fs4::available_space(destination_dir)?;
    let required = required_bytes + required_bytes / 10; // 10% buffer
    if available < required {
        return Err(DownloadError::InsufficientDiskSpace {
            available,
            required,
        });
    }
    Ok(())
}

fn preallocate_file(file: &File, total_size: Option<u64>) -> Result<()> {
    let Some(total_size) = total_size else {
        return Ok(());
    };

    match file.allocate(total_size) {
        Ok(()) => Ok(()),
        Err(error) => match error.raw_os_error() {
            Some(38 | 45 | 95 | 524) => {
                file.set_len(total_size)?;
                Ok(())
            }
            _ => Err(error.into()),
        },
    }
}

#[cfg(unix)]
fn write_once_at(file: &File, buffer: &[u8], offset: u64) -> std::io::Result<usize> {
    use std::os::unix::fs::FileExt;

    file.write_at(buffer, offset)
}

#[cfg(windows)]
fn write_once_at(file: &File, buffer: &[u8], offset: u64) -> std::io::Result<usize> {
    use std::os::windows::fs::FileExt;

    file.seek_write(buffer, offset)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use ntest::timeout;
    use tempfile::tempdir;

    use super::{finalize_temp_file, open_download_file, reset_download_file, write_all_at};

    type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

    #[timeout(30_000)]
    #[test]
    fn open_download_file_sizes_known_length() -> TestResult {
        let temp = tempdir()?;
        let path = temp.path().join("download.part");

        let file = open_download_file(&path, Some(1024 * 1024))?;

        assert_eq!(file.metadata()?.len(), 1024 * 1024);
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn reset_download_file_restores_target_length() -> TestResult {
        let temp = tempdir()?;
        let path = temp.path().join("download.part");
        let file = open_download_file(&path, Some(4096))?;

        file.set_len(128)?;
        reset_download_file(&file, Some(8192))?;

        assert_eq!(file.metadata()?.len(), 8192);
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn write_after_preallocation_preserves_file() -> TestResult {
        let temp = tempdir()?;
        let path = temp.path().join("download.part");
        let file = open_download_file(&path, Some(4096))?;

        write_all_at(&file, b"test data", 0)?;

        assert_eq!(file.metadata()?.len(), 4096);
        let bytes = fs::read(path)?;
        assert_eq!(&bytes[..9], b"test data");
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn finalize_temp_file_refuses_to_overwrite_destination() -> TestResult {
        let temp = tempdir()?;
        let source = temp.path().join("download.part");
        let destination = temp.path().join("download.bin");
        fs::write(&source, b"new")?;
        fs::write(&destination, b"existing")?;

        let result = finalize_temp_file(&source, &destination);

        assert!(result.is_err());
        assert_eq!(fs::read(&destination)?, b"existing");
        assert_eq!(fs::read(&source)?, b"new");
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn finalize_temp_file_moves_completed_download() -> TestResult {
        let temp = tempdir()?;
        let source = temp.path().join("download.part");
        let destination = temp.path().join("download.bin");
        fs::write(&source, b"complete")?;

        finalize_temp_file(&source, &destination)?;

        assert!(!source.exists());
        assert_eq!(fs::read(&destination)?, b"complete");
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn finalize_temp_file_accepts_existing_identical_destination() -> TestResult {
        let temp = tempdir()?;
        let source = temp.path().join("download.part");
        let destination = temp.path().join("download.bin");
        fs::write(&source, b"complete")?;
        fs::write(&destination, b"complete")?;

        finalize_temp_file(&source, &destination)?;

        assert!(!source.exists());
        assert_eq!(fs::read(&destination)?, b"complete");
        Ok(())
    }
}
