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

    preallocate_file(&file, total_size)?;
    Ok(file)
}

pub fn reset_download_file(file: &File, total_size: Option<u64>) -> Result<()> {
    file.set_len(0)?;
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

// ── Windows Fast Preallocation & Sparse File Support ──
#[cfg(windows)]
#[repr(C)]
struct Luid {
    low_part: u32,
    high_part: i32,
}

#[cfg(windows)]
#[repr(C)]
struct LuidAndAttributes {
    luid: Luid,
    attributes: u32,
}

#[cfg(windows)]
#[repr(C)]
struct TokenPrivileges {
    privilege_count: u32,
    privileges: [LuidAndAttributes; 1],
}

#[cfg(windows)]
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
    fn SetFileValidData(h_file: isize, valid_data_length: i64) -> i32;
    fn GetCurrentProcess() -> isize;
    fn OpenProcessToken(process_handle: isize, desired_access: u32, token_handle: *mut isize) -> i32;
    fn CloseHandle(handle: isize) -> i32;
    fn LookupPrivilegeValueW(lp_system_name: *const u16, lp_name: *const u16, lp_luid: *mut Luid) -> i32;
    fn AdjustTokenPrivileges(
        token_handle: isize,
        disable_all_privileges: i32,
        new_state: *const TokenPrivileges,
        buffer_length: u32,
        previous_state: *mut std::ffi::c_void,
        return_length: *mut u32,
    ) -> i32;
}

/// Check and acquire `SeManageVolumePrivilege` once per process.
/// This privilege is required for instant, zero-filling-free `SetFileValidData` allocation.
#[cfg(windows)]
static HAS_MANAGE_VOLUME_PRIVILEGE: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
    unsafe {
        const TOKEN_ADJUST_PRIVILEGES: u32 = 0x0020;
        const TOKEN_QUERY: u32 = 0x0008;
        const SE_PRIVILEGE_ENABLED: u32 = 0x00000002;

        let mut token: isize = 0;
        if OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token) == 0 {
            return false;
        }

        let name: Vec<u16> = "SeManageVolumePrivilege\0".encode_utf16().collect();
        let mut luid = Luid { low_part: 0, high_part: 0 };
        if LookupPrivilegeValueW(std::ptr::null(), name.as_ptr(), &mut luid) == 0 {
            CloseHandle(token);
            return false;
        }

        let tp = TokenPrivileges {
            privilege_count: 1,
            privileges: [LuidAndAttributes {
                luid,
                attributes: SE_PRIVILEGE_ENABLED,
            }],
        };

        let res = AdjustTokenPrivileges(
            token,
            0,
            &tp,
            std::mem::size_of::<TokenPrivileges>() as u32,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        let err = GetLastError();
        CloseHandle(token);
        res != 0 && err == 0
    }
});

/// Attempt instantaneous disk preallocation on Windows without zero-filling or fragmentation.
/// Requires `SeManageVolumePrivilege` (enabled when run as admin or with volume management rights).
#[cfg(windows)]
fn try_fast_preallocate_win(file: &File, total_size: u64) -> bool {
    use std::os::windows::io::AsRawHandle;

    if !*HAS_MANAGE_VOLUME_PRIVILEGE {
        return false;
    }

    if file.set_len(total_size).is_err() {
        return false;
    }

    let handle = file.as_raw_handle() as isize;
    let res = unsafe { SetFileValidData(handle, total_size as i64) };
    res != 0
}

/// Configure the file as sparse to avoid zero-filling stalls on non-sequential chunk writes.
#[cfg(windows)]
pub fn set_sparse_file(file: &File) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;

    const FSCTL_SET_SPARSE: u32 = 0x000900C4;
    let handle = file.as_raw_handle();
    let mut bytes_returned: u32 = 0;

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

    #[cfg(windows)]
    {
        // 1. Try instantaneous SetFileValidData (requires SeManageVolumePrivilege).
        // If permitted, this sets ValidDataLength == EOF in 0 ms without writing
        // zeroes to disk and without file fragmentation.
        if try_fast_preallocate_win(file, total_size) {
            return Ok(());
        }

        // 2. Fall back to sparse file mode on Windows to avoid synchronous zero-fill stalls
        // on non-sequential chunk writes.
        let _ = set_sparse_file(file);
    }

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
mod media;
pub use disk_detect::detect_all_disk_types;
pub use disk_detect::detect_disk_type;
pub use media::MediaOverrides;
pub use media::is_network_filesystem;
pub use media::is_usable_override_key;
pub use media::lookup_media_override;
pub use media::normalize_media_path;
pub use media::parse_mountinfo;
pub use media::path_is_within;

// ── Tests ─────────────────────────────────────────────

#[cfg(test)]
mod tests;
