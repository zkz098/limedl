use std::fs;
use std::io::{self, ErrorKind};
use std::path::Path;

use ntest::timeout;
use tempfile::tempdir;

use super::{
    check_disk_space, cleanup_finalizing_paths, files_have_same_content, finalize_temp_file,
    open_download_file, preallocate_file, reservation_error, reset_download_file, set_sparse_file,
    unique_finalizing_path, write_all_at, write_all_vectored_at,
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
        DownloadError::InsufficientDiskSpace {
            available: a,
            required: r,
        } => {
            // Disk free space may fluctuate between query and check;
            // verify the invariant rather than exact values.
            assert!(a < r, "available ({a}) must be less than required ({r})");
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

    let file = fs::OpenOptions::new().read(true).open(&path)?;

    let result = preallocate_file(&file, Some(4096));
    assert!(result.is_err());
    Ok(())
}

// ── reservation_error ─────────────────────────────

/// 4 GiB in bytes — a FAT32 volume's per-file cap.
const GIB: u64 = 1024 * 1024 * 1024;

/// A volume that refuses the file itself has to read differently from one that
/// is merely full: "free up space" is useless advice against a size limit that
/// does not move no matter how much is deleted.
#[timeout(30_000)]
#[test]
fn reservation_error_reports_a_per_file_size_limit() {
    let too_large = io::Error::from(ErrorKind::FileTooLarge);
    assert_eq!(
        reservation_error(too_large, 5 * GIB, None).kind(),
        "file_too_large",
        "EFBIG / ERROR_FILE_TOO_LARGE names the limit directly"
    );

    // Some volumes answer an over-limit reservation with "disk full" even though
    // they have room; the free-space reading is what tells the two apart.
    let disk_full = io::Error::from(ErrorKind::StorageFull);
    assert_eq!(
        reservation_error(disk_full, 5 * GIB, Some(64 * GIB)).kind(),
        "file_too_large",
        "a volume with room for the whole file is refusing the size, not the space"
    );
}

/// The two neighbours of that case keep their own error, and nothing is claimed
/// about a volume whose free space could not be read at all.
#[timeout(30_000)]
#[test]
fn reservation_error_keeps_space_shortage_and_unrelated_failures() {
    let disk_full = io::Error::from(ErrorKind::StorageFull);
    match reservation_error(disk_full, 5 * GIB, Some(GIB)) {
        DownloadError::InsufficientDiskSpace {
            available,
            required,
        } => assert_eq!((available, required), (GIB, 5 * GIB)),
        other => panic!("expected InsufficientDiskSpace, got {other:?}"),
    }

    // No free-space reading means no verdict on the volume.
    let unknown_space = io::Error::from(ErrorKind::StorageFull);
    assert_eq!(reservation_error(unknown_space, 5 * GIB, None).kind(), "io");

    // A read-only volume is not a size limit, however much room it has.
    let read_only = io::Error::from(ErrorKind::ReadOnlyFilesystem);
    assert_eq!(
        reservation_error(read_only, 5 * GIB, Some(64 * GIB)).kind(),
        "io"
    );
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

    let file = fs::OpenOptions::new().read(true).open(&path)?;

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

// ── finalize_temp_file cross-device (Unix) ────────

/// A pair of temp directories on two different filesystems, or `None` when
/// the host has only one writable volume (macOS CI, single-mount containers).
///
/// The drive-letter test above only ever runs on Windows; Linux CI has
/// `/dev/shm` (tmpfs) next to `/tmp` (ext4), so the copy fallback that
/// `std::fs::rename` cannot handle is exercised there.
#[cfg(unix)]
fn cross_device_dirs() -> Option<(tempfile::TempDir, tempfile::TempDir)> {
    use std::os::unix::fs::MetadataExt;

    let primary = tempfile::tempdir().ok()?;
    let primary_device = fs::metadata(primary.path()).ok()?.dev();
    for mount in ["/dev/shm", "/run/shm", "/var/tmp"] {
        let Ok(other) = tempfile::tempdir_in(mount) else {
            continue;
        };
        if fs::metadata(other.path()).ok()?.dev() != primary_device {
            return Some((primary, other));
        }
    }
    None
}

/// No `.finalizing.*` staging file may be left next to the published file.
#[cfg(unix)]
fn assert_no_staging_files(dir: &Path, destination_name: &str) {
    let prefix = format!("{destination_name}.finalizing.");
    let leftovers: Vec<String> = fs::read_dir(dir)
        .expect("read destination dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(&prefix))
        .collect();
    assert!(
        leftovers.is_empty(),
        "staging residue left behind: {leftovers:?}"
    );
}

/// Cross-device finalize must copy the whole file and only then publish it:
/// the destination is byte-identical, the temp source is gone, and no staging
/// file is left behind. On a same-device rename this is trivial; on the copy
/// fallback a short write or an early rename would leave a truncated file
/// behind the user's final name.
#[timeout(60_000)]
#[cfg(unix)]
#[test]
fn finalize_temp_file_cross_device_copy_is_byte_identical() -> TestResult {
    let Some((primary, secondary)) = cross_device_dirs() else {
        return Ok(());
    };

    let source = secondary.path().join("download.part");
    let destination = primary.path().join("download.bin");
    // Larger than the 1 MiB copy buffer so the read/write loop takes several
    // passes and leaves a ragged tail.
    let content: Vec<u8> = (0..(3 * 1024 * 1024 + 123u32))
        .map(|i| (i % 251) as u8)
        .collect();
    fs::write(&source, &content)?;

    finalize_temp_file(&source, &destination)?;

    assert!(
        !source.exists(),
        "source must be removed once the copy is published"
    );
    assert_eq!(
        fs::read(&destination)?,
        content,
        "destination must be byte-identical"
    );
    assert_no_staging_files(primary.path(), "download.bin");
    Ok(())
}

/// When the cross-device fallback cannot even create its staging file, the
/// finalize must fail without touching the source or the destination: a failed
/// publish may never lose the only copy of the data.
#[timeout(60_000)]
#[cfg(unix)]
#[test]
fn finalize_temp_file_cross_device_keeps_source_when_staging_is_exhausted() -> TestResult {
    let Some((primary, secondary)) = cross_device_dirs() else {
        return Ok(());
    };

    let source = secondary.path().join("download.part");
    let destination = primary.path().join("download.bin");
    let content = b"content that must survive a failed finalize";
    fs::write(&source, content)?;

    // Occupy every candidate staging slot so `unique_finalizing_path` gives up
    // before the copy starts (deterministic, unlike a permissions race).
    let process_id = std::process::id();
    for attempt in 0..1000u16 {
        let name = format!("download.bin.finalizing.{process_id}.{attempt}.tmp");
        fs::write(primary.path().join(name), b"")?;
    }

    let result = finalize_temp_file(&source, &destination);

    assert!(
        result.is_err(),
        "an exhausted staging pool must fail the finalize"
    );
    assert_eq!(
        fs::read(&source)?,
        content,
        "source must survive a failed finalize"
    );
    assert!(
        !destination.exists(),
        "no destination may be published from a failed copy"
    );
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
