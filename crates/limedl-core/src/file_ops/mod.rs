// ── File Allocation ──────────────────────────────────

use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, BufReader, ErrorKind, Read, Write},
    path::Path,
};

use fs4::FileExt;

use tracing;

use super::error::{DownloadError, Result, io_error_with_path};

pub fn open_download_file(path: &Path, total_size: Option<u64>) -> Result<File> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| io_error_with_path(e, parent.to_string_lossy()))?;
    }

    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| io_error_with_path(e, path.to_string_lossy()))?;

    let _ = set_sparse_file(&file);
    preallocate_file(&file, total_size)?;
    Ok(file)
}

pub fn reset_download_file(file: &File, total_size: Option<u64>) -> Result<()> {
    file.set_len(0)?;
    let _ = set_sparse_file(file);
    preallocate_file(file, total_size)
}

pub fn finalize_temp_file(temp_path: &Path, destination_path: &Path) -> Result<()> {
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
            // Fallback: copy via staging path in destination_path's parent directory
            // when source and destination reside on different mount points / drive letters.
            let staging_path = unique_finalizing_path(destination_path)?;
            let mut source = File::open(temp_path)?;
            let mut destination = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&staging_path)
                .map_err(DownloadError::Io)?;

            if let Err(error) = copy_file_buffered(&mut source, &mut destination) {
                drop(destination);
                drop(source);
                if let Err(e) = fs::remove_file(&staging_path) {
                    tracing::warn!(
                        "Failed to clean up staging file {}: {e}",
                        staging_path.display()
                    );
                }
                return Err(error.into());
            }
            destination.flush()?;
            destination.sync_all()?;
            drop(destination);
            drop(source);

            // staging_path and destination_path are in the exact same directory,
            // so renaming staging_path to destination_path is guaranteed to be a same-volume atomic move.
            if let Err(error) = std::fs::rename(&staging_path, destination_path) {
                if destination_path.exists() {
                    if files_have_same_content(&staging_path, destination_path)? {
                        let _ = fs::remove_file(&staging_path);
                        let _ = fs::remove_file(temp_path);
                        return Ok(());
                    }
                    let _ = fs::remove_file(&staging_path);
                    return Err(destination_exists_error(destination_path));
                }
                let _ = fs::remove_file(&staging_path);
                return Err(error.into());
            }

            let _ = fs::remove_file(temp_path);
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

/// Copy file contents using a 1 MB heap-allocated buffer for high-throughput disk copy.
fn copy_file_buffered(source: &mut File, dest: &mut File) -> io::Result<u64> {
    let mut buffer = vec![0u8; 1024 * 1024];
    let mut total = 0u64;
    loop {
        let bytes_read = match source.read(&mut buffer) {
            Ok(0) => return Ok(total),
            Ok(n) => n,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        {
            let mut buf = &buffer[..bytes_read];
            while !buf.is_empty() {
                match dest.write(buf) {
                    Ok(0) => {
                        return Err(io::Error::new(ErrorKind::WriteZero, "write returned zero"));
                    }
                    Ok(n) => buf = &buf[n..],
                    Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                }
            }
        }
        total += bytes_read as u64;
    }
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

    let mut left = BufReader::with_capacity(1024 * 1024, left);
    let mut right = BufReader::with_capacity(1024 * 1024, right);
    let mut left_buffer = vec![0u8; 1024 * 1024];
    let mut right_buffer = vec![0u8; 1024 * 1024];
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

    for entry in
        fs::read_dir(parent).map_err(|e| io_error_with_path(e, parent.to_string_lossy()))?
    {
        let entry = entry.map_err(|e| io_error_with_path(e, parent.to_string_lossy()))?;
        let candidate_name = entry.file_name();
        if candidate_name
            .to_string_lossy()
            .starts_with(&prefix.to_string_lossy().to_string())
            && let Err(e) = fs::remove_file(entry.path())
        {
            tracing::warn!(
                "Failed to clean up finalizing file {}: {e}",
                entry.path().display()
            );
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

pub fn write_all_at(file: &File, mut buffer: &[u8], mut offset: u64) -> Result<()> {
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

/// Write a sequence of buffer slices to `file` at `offset`, using vectored I/O (`pwritev`)
/// on Unix platforms to eliminate memory allocations and copies, with single-syscall
/// coalescing fallback on other platforms.
#[cfg(unix)]
pub fn write_all_vectored_at(file: &File, bufs: &[&[u8]], mut offset: u64) -> Result<()> {
    use std::os::unix::io::AsRawFd;

    if bufs.is_empty() {
        return Ok(());
    }
    if bufs.len() == 1 {
        return write_all_at(file, bufs[0], offset);
    }

    let fd = file.as_raw_fd();
    let mut slices = bufs;
    let mut first_slice_offset = 0usize;

    while !slices.is_empty() {
        // Skip leading empty slices
        while let Some(first) = slices.first() {
            if first.len() <= first_slice_offset {
                slices = &slices[1..];
                first_slice_offset = 0;
            } else {
                break;
            }
        }
        if slices.is_empty() {
            break;
        }

        // Limit the batch to 1024 (POSIX UIO_MAXIOV limit)
        let batch_len = slices.len().min(1024);
        let mut iov: Vec<libc::iovec> = Vec::with_capacity(batch_len);

        for (i, slice) in slices[..batch_len].iter().enumerate() {
            let data = if i == 0 {
                &slice[first_slice_offset..]
            } else {
                *slice
            };
            if !data.is_empty() {
                iov.push(libc::iovec {
                    iov_base: data.as_ptr() as *mut libc::c_void,
                    iov_len: data.len(),
                });
            }
        }

        if iov.is_empty() {
            slices = &slices[batch_len..];
            first_slice_offset = 0;
            continue;
        }

        let res = unsafe {
            libc::pwritev(
                fd,
                iov.as_ptr(),
                iov.len() as libc::c_int,
                offset as libc::off_t,
            )
        };

        if res < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(DownloadError::Io(err));
        }

        if res == 0 {
            return Err(DownloadError::InvalidResponse(String::from(
                "failed to write download data (pwritev returned 0)",
            )));
        }

        let mut written = res as usize;
        offset += written as u64;

        // Advance through written slices
        while written > 0 && !slices.is_empty() {
            let current_len = slices[0].len() - first_slice_offset;
            if written >= current_len {
                written -= current_len;
                slices = &slices[1..];
                first_slice_offset = 0;
            } else {
                first_slice_offset += written;
                written = 0;
            }
        }
    }
    Ok(())
}

/// Write a sequence of buffer slices to `file` at `offset`, using vectored I/O (`pwritev`)
/// on Unix platforms to eliminate memory allocations and copies, with single-syscall
/// coalescing fallback on other platforms.
#[cfg(not(unix))]
pub fn write_all_vectored_at(file: &File, bufs: &[&[u8]], offset: u64) -> Result<()> {
    if bufs.is_empty() {
        return Ok(());
    }
    if bufs.len() == 1 {
        return write_all_at(file, bufs[0], offset);
    }

    // Windows / non-Unix fallback: coalesce slices into a contiguous buffer
    // to perform a single seek_write syscall.
    let total_len: usize = bufs.iter().map(|b| b.len()).sum();
    let mut combined = Vec::with_capacity(total_len);
    for b in bufs {
        combined.extend_from_slice(b);
    }
    write_all_at(file, &combined, offset)
}

/// Configure the file as sparse to avoid zero-filling stalls on non-sequential chunk writes.
#[cfg(windows)]
pub fn set_sparse_file(file: &File) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;

    const FSCTL_SET_SPARSE: u32 = 0x000900C4;
    let handle = file.as_raw_handle();
    let mut bytes_returned: u32 = 0;

    unsafe extern "system" {
        fn DeviceIoControl(
            h_device: isize,
            dw_io_control_code: u32,
            lp_in_buffer: *const std::ffi::c_void,
            n_in_buffer_size: u32,
            lp_out_buffer: *mut std::ffi::c_void,
            n_out_buffer_size: u32,
            lp_bytes_returned: *mut u32,
            lp_overlapped: *mut std::ffi::c_void,
        ) -> i32;
        fn GetLastError() -> u32;
    }

    let success = unsafe {
        DeviceIoControl(
            handle as isize,
            FSCTL_SET_SPARSE,
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            0,
            &mut bytes_returned,
            std::ptr::null_mut(),
        )
    };

    if success == 0 {
        let err = unsafe { GetLastError() };
        // ERROR_INVALID_FUNCTION (1), ERROR_NOT_SUPPORTED (50), ERROR_INVALID_PARAMETER (87)
        // are returned when the filesystem (e.g. FAT32, exFAT, or certain network mounts)
        // does not support sparse files. We gracefully treat this as non-fatal.
        if err == 1 || err == 50 || err == 87 {
            return Ok(());
        }
        return Err(io::Error::from_raw_os_error(err as i32));
    }

    Ok(())
}

/// Configure the file as sparse on Unix platforms (no-op as Unix filesystems natively support sparse holes).
#[cfg(not(windows))]
pub fn set_sparse_file(_file: &File) -> io::Result<()> {
    Ok(())
}

/// Checks that the destination directory has enough free space for the download.
/// `required_bytes` is the total file size. We require 10% buffer above that.
pub fn check_disk_space(destination_dir: &Path, required_bytes: u64) -> Result<()> {
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

    // On Windows, enable sparse attribute first so out-of-order chunk writes
    // (e.g. BT pieces or multi-connection HTTP chunks) never trigger synchronous zero-filling stalls.
    #[cfg(windows)]
    let _ = set_sparse_file(file);

    match file.allocate(total_size) {
        Ok(()) => Ok(()),
        Err(error) => match error.raw_os_error() {
            // 1 = EPERM, 22 = EINVAL, 38 = ENOSYS, 45 = ENOTSUP, 95 = EOPNOTSUPP, 524 = ENOTSUP (glibc)
            Some(1 | 22 | 38 | 45 | 95 | 524) => {
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

mod disk_detect;
pub use disk_detect::detect_disk_type;
pub use disk_detect::detect_all_disk_types;

// ── Tests ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use ntest::timeout;
    use tempfile::tempdir;

    use super::{
        check_disk_space, cleanup_finalizing_paths,
        files_have_same_content, finalize_temp_file, open_download_file, preallocate_file,
        reset_download_file, set_sparse_file, unique_finalizing_path, write_all_at, write_all_vectored_at,
    };
    use crate::error::DownloadError;

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

    // ── check_disk_space ──────────────────────────────

    #[timeout(30_000)]
    #[test]
    fn check_disk_space_insufficient() -> TestResult {
        let temp = tempdir()?;
        let available = fs4::available_space(temp.path())?;

        // Request more than available (incl. 10% buffer) → error
        let required_bytes = available + 1;
        let result = check_disk_space(temp.path(), required_bytes);

        assert!(result.is_err());
        match result.unwrap_err() {
            DownloadError::InsufficientDiskSpace { available: a, required: r } => {
                // Disk free space may fluctuate between query and check;
                // verify the invariant rather than exact values.
                assert!(
                    a < r,
                    "available ({a}) must be less than required ({r})"
                );
                assert_eq!(
                    r,
                    required_bytes + required_bytes / 10,
                    "required should include 10% buffer"
                );
            }
            other => panic!("expected InsufficientDiskSpace, got {other:?}"),
        }
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn check_disk_space_sufficient() -> TestResult {
        let temp = tempdir()?;

        // 1 byte required — way less than any real filesystem has available
        let result = check_disk_space(temp.path(), 1);

        assert!(result.is_ok());
        Ok(())
    }

    // ── preallocate_file ──────────────────────────────

    #[timeout(30_000)]
    #[test]
    fn preallocate_file_skips_on_none() -> TestResult {
        let temp = tempdir()?;
        let path = temp.path().join("none.dat");
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)?;

        assert!(preallocate_file(&file, None).is_ok());
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn preallocate_file_fails_on_readonly_fd() -> TestResult {
        let temp = tempdir()?;
        let path = temp.path().join("readonly.dat");
        // Create writable, close, reopen read-only
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)?;
        drop(file);

        let file = fs::OpenOptions::new()
            .read(true)
            .open(&path)?;

        let result = preallocate_file(&file, Some(4096));
        assert!(result.is_err());
        Ok(())
    }

    // ── write_all_at ──────────────────────────────────

    #[timeout(30_000)]
    #[test]
    fn write_all_at_empty_buffer_succeeds() -> TestResult {
        let temp = tempdir()?;
        let path = temp.path().join("empty.dat");
        let file = open_download_file(&path, Some(1024))?;

        write_all_at(&file, b"", 0)?;
        // File should remain at the preallocated size
        assert_eq!(file.metadata()?.len(), 1024);
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn write_all_at_fails_on_readonly_fd() -> TestResult {
        let temp = tempdir()?;
        let path = temp.path().join("write_ro.dat");
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)?;
        drop(file);

        let file = fs::OpenOptions::new()
            .read(true)
            .open(&path)?;

        let result = write_all_at(&file, b"data", 0);
        assert!(result.is_err());
        Ok(())
    }

    // ── finalize_temp_file cross-device ───────────────

    #[timeout(30_000)]
    #[test]
    fn finalize_temp_file_cross_device_copy() -> TestResult {
        // Find a second writable volume to trigger CrossesDevices fallback
        let drives: Vec<String> = (b'C'..=b'Z')
            .map(|c| format!("{}:\\", c as char))
            .filter(|p| Path::new(p).exists())
            .collect();

        if drives.len() < 2 {
            // Single-drive system — can't test cross-device rename
            return Ok(());
        }

        let dir_a = tempfile::tempdir_in(&drives[0])?;
        let dir_b = tempfile::tempdir_in(&drives[1])?;

        let source = dir_a.path().join("source.dat");
        let destination = dir_b.path().join("dest.dat");
        fs::write(&source, b"cross-device content")?;

        finalize_temp_file(&source, &destination)?;

        assert!(!source.exists(), "source should be removed");
        assert_eq!(fs::read(&destination)?, b"cross-device content");
        Ok(())
    }

    // ── files_have_same_content ───────────────────────

    #[timeout(30_000)]
    #[test]
    fn files_have_same_content_different_sizes() -> TestResult {
        let temp = tempdir()?;
        let a = temp.path().join("a.txt");
        let b = temp.path().join("b.txt");
        fs::write(&a, b"short")?;
        fs::write(&b, b"longer content")?;

        assert!(!files_have_same_content(&a, &b)?);
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn files_have_same_content_same_size_different() -> TestResult {
        let temp = tempdir()?;
        let left = temp.path().join("left.bin");
        let right = temp.path().join("right.bin");
        fs::write(&left, b"AAAA")?;
        fs::write(&right, b"BBBB")?;

        assert!(!files_have_same_content(&left, &right)?);
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn files_have_same_content_identical() -> TestResult {
        let temp = tempdir()?;
        let a = temp.path().join("a.txt");
        let b = temp.path().join("b.txt");
        let content = b"The quick brown fox jumps over the lazy dog 1234567890";
        fs::write(&a, content)?;
        fs::write(&b, content)?;

        assert!(files_have_same_content(&a, &b)?);
        Ok(())
    }

    // ── unique_finalizing_path ────────────────────────

    #[timeout(30_000)]
    #[test]
    fn unique_finalizing_path_no_existing_files() -> TestResult {
        let temp = tempdir()?;
        let dest = temp.path().join("myfile.bin");
        let staging = unique_finalizing_path(&dest)?;

        let display = staging.to_string_lossy();
        assert!(display.contains("myfile.bin.finalizing."));
        assert!(!staging.exists());
        assert_eq!(staging.parent(), Some(temp.path()));
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn unique_finalizing_path_without_extension() -> TestResult {
        let temp = tempdir()?;
        let dest = temp.path().join("myfile");
        let staging = unique_finalizing_path(&dest)?;

        let display = staging.to_string_lossy();
        assert!(display.contains("myfile.finalizing."));
        assert!(!staging.exists());
        Ok(())
    }

    // ── cleanup_finalizing_paths ────────────────────

    #[timeout(30_000)]
    #[test]
    fn cleanup_finalizing_paths_removes_matching() -> TestResult {
        let temp = tempdir()?;
        let dest = temp.path().join("myfile.bin");

        // Create some finalizing files (matching and non-matching prefixes)
        let f1 = temp.path().join("myfile.bin.finalizing.1234.0.tmp");
        let f2 = temp.path().join("myfile.bin.finalizing.1234.1.tmp");
        let f3 = temp.path().join("otherfile.finalizing.1234.0.tmp");
        let f4 = temp.path().join("unrelated.txt");
        fs::write(&f1, b"")?;
        fs::write(&f2, b"")?;
        fs::write(&f3, b"")?;
        fs::write(&f4, b"")?;

        cleanup_finalizing_paths(&dest)?;

        assert!(!f1.exists(), "matching finalizing file 1 should be removed");
        assert!(!f2.exists(), "matching finalizing file 2 should be removed");
        assert!(f3.exists(), "non-matching finalizing file should remain");
        assert!(f4.exists(), "unrelated file should remain");
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn cleanup_finalizing_paths_no_parent() -> TestResult {
        // A path with no parent (empty) should hit the early-return and be a no-op
        cleanup_finalizing_paths(Path::new(""))?;
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn cleanup_finalizing_paths_read_dir_fails() -> TestResult {
        // Non-existent parent directory → read_dir fails → error returned
        let nonexistent = Path::new(r"\__nonexistent_test_dir__\file.bin");
        let result = cleanup_finalizing_paths(nonexistent);
        assert!(result.is_err(), "expected error for non-existent parent");
        Ok(())
    }

    // ── unique_finalizing_path exhaustion ──────────

    #[timeout(30_000)]
    #[test]
    fn unique_finalizing_path_exhaustion() -> TestResult {
        let temp = tempdir()?;
        let dest = temp.path().join("myfile.bin");
        let pid = std::process::id();

        // Create 1000 files matching all attempt slots (0..1000) to exhaust the loop
        for i in 0..1000u16 {
            let name = format!("myfile.bin.finalizing.{pid}.{i}.tmp");
            let path = temp.path().join(&name);
            fs::write(&path, b"")?;
        }

        let result = unique_finalizing_path(&dest);
        assert!(result.is_err(), "expected exhaustion error");
        match result.unwrap_err() {
            DownloadError::Io(io_err) => {
                assert_eq!(io_err.kind(), std::io::ErrorKind::AlreadyExists);
            }
            other => panic!("expected Io(AlreadyExists), got {other:?}"),
        }
        Ok(())
    }

    // ── write_all_vectored_at & sparse tests ──────────

    #[timeout(30_000)]
    #[test]
    fn write_all_vectored_at_multiple_slices() -> TestResult {
        let temp = tempdir()?;
        let path = temp.path().join("vectored.dat");
        let file = open_download_file(&path, Some(1024))?;

        let slices: Vec<&[u8]> = vec![b"hello ", b"vectored ", b"world!"];
        write_all_vectored_at(&file, &slices, 10)?;

        let bytes = fs::read(&path)?;
        assert_eq!(&bytes[10..31], b"hello vectored world!");
        assert_eq!(bytes.len(), 1024);
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn write_all_vectored_at_empty_and_single() -> TestResult {
        let temp = tempdir()?;
        let path = temp.path().join("vectored_empty.dat");
        let file = open_download_file(&path, Some(512))?;

        // Empty bufs slice should be no-op
        write_all_vectored_at(&file, &[], 0)?;

        // Array with empty slices and one data slice
        let slices: Vec<&[u8]> = vec![b"", b"single_data", b""];
        write_all_vectored_at(&file, &slices, 0)?;

        let bytes = fs::read(&path)?;
        assert_eq!(&bytes[..11], b"single_data");
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn open_download_file_sparse_and_high_offset_write() -> TestResult {
        let temp = tempdir()?;
        let path = temp.path().join("sparse_high_offset.part");
        // Preallocate 10 MB file
        let file = open_download_file(&path, Some(10 * 1024 * 1024))?;
        assert_eq!(file.metadata()?.len(), 10 * 1024 * 1024);

        // Write at high offset (5 MB and 9 MB)
        let middle_offset = 5 * 1024 * 1024;
        write_all_at(&file, b"middle_sparse_chunk", middle_offset)?;

        let end_offset = 9 * 1024 * 1024;
        let slices: Vec<&[u8]> = vec![b"end_", b"sparse_", b"chunk"];
        write_all_vectored_at(&file, &slices, end_offset)?;

        let bytes = fs::read(&path)?;
        assert_eq!(bytes.len(), 10 * 1024 * 1024);
        assert_eq!(
            &bytes[middle_offset as usize..(middle_offset as usize + 19)],
            b"middle_sparse_chunk"
        );
        assert_eq!(
            &bytes[end_offset as usize..(end_offset as usize + 16)],
            b"end_sparse_chunk"
        );
        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn set_sparse_file_on_regular_file() -> TestResult {
        let temp = tempdir()?;
        let path = temp.path().join("sparse_test.dat");
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .read(true)
            .open(&path)?;

        assert!(set_sparse_file(&file).is_ok());
        Ok(())
    }
}
