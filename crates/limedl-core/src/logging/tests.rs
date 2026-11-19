use std::time::{Duration, SystemTime};

use tempfile::tempdir;

use super::*;

fn write_file(path: &Path, content: &str) {
    fs::write(path, content).expect("write test log file");
}

/// Move a file's mtime into the past so age-based retention can be tested
/// without waiting days (`File::set_modified`, stable since Rust 1.75).
fn age_file(path: &Path, age: Duration) {
    let file = fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("open test log file");
    file.set_modified(SystemTime::now() - age)
        .expect("set modified time");
}

#[test]
fn resolve_log_file_path_uses_state_dir_or_configured_path() {
    let state = tempdir().expect("tempdir");
    assert_eq!(
        resolve_log_file_path(&LogSettings::default(), state.path()),
        state.path().join("logs").join("limedl.log")
    );

    let custom = LogSettings {
        file_path: "  /var/log/limedl/custom.log  ".to_string(),
        ..Default::default()
    };
    assert_eq!(
        resolve_log_file_path(&custom, state.path()),
        PathBuf::from("/var/log/limedl/custom.log"),
        "a configured path wins over the state-dir default and is trimmed"
    );
}

#[test]
fn log_dir_for_creates_the_configured_parent() {
    let tmp = tempdir().expect("tempdir");
    let custom = tmp.path().join("nested").join("logs").join("limedl.log");
    let settings = LogSettings {
        file_path: custom.to_string_lossy().into_owned(),
        ..Default::default()
    };

    let dir = log_dir_for(&settings, tmp.path()).expect("create log dir");

    assert_eq!(dir, custom.parent().unwrap());
    assert!(dir.is_dir(), "the full parent chain must be created");
}

#[test]
fn find_rotated_logs_filters_and_sorts_descending() {
    let tmp = tempdir().expect("tempdir");
    let current = tmp.path().join("limedl.log");
    for name in [
        "limedl.log",
        "limedl.1.log",
        "limedl.2.log",
        "limedl.x.log",
        "limedl.1.txt",
        "other.1.log",
    ] {
        write_file(&tmp.path().join(name), name);
    }

    let logs = find_rotated_logs(&current);
    let found: Vec<String> = logs
        .iter()
        .map(|(path, num)| format!("{}={num}", path.file_name().unwrap().to_string_lossy()))
        .collect();

    assert_eq!(
        found,
        vec!["limedl.2.log=2", "limedl.1.log=1"],
        "only `limedl.<n>.log` counts, and rotation numbers sort descending"
    );
    assert!(
        find_rotated_logs(Path::new("limedl")).is_empty(),
        "a path without an extension has no rotated siblings"
    );
}

/// Renaming must go from the highest rotation number down. Ascending, the
/// first rename would clobber `.2.log` before it was moved to `.3.log` and
/// silently destroy a log file.
#[test]
fn rotate_startup_logs_shifts_without_losing_content() {
    let tmp = tempdir().expect("tempdir");
    let current = tmp.path().join("limedl.log");
    write_file(&current, "current");
    write_file(&tmp.path().join("limedl.1.log"), "one");
    write_file(&tmp.path().join("limedl.2.log"), "two");

    rotate_startup_logs(&current);

    assert!(!current.exists(), "the current log becomes the .1 file");
    assert_eq!(
        fs::read_to_string(tmp.path().join("limedl.1.log")).unwrap(),
        "current"
    );
    assert_eq!(
        fs::read_to_string(tmp.path().join("limedl.2.log")).unwrap(),
        "one"
    );
    assert_eq!(
        fs::read_to_string(tmp.path().join("limedl.3.log")).unwrap(),
        "two",
        "the oldest rotated file must move to .3, not be overwritten"
    );
}

#[test]
fn cleanup_by_count_keeps_exactly_the_limit() {
    let tmp = tempdir().expect("tempdir");
    let current = tmp.path().join("limedl.log");
    for n in 1..=4u32 {
        write_file(&tmp.path().join(format!("limedl.{n}.log")), &format!("{n}"));
    }

    cleanup_by_count(&current, 2);

    assert!(tmp.path().join("limedl.1.log").exists());
    assert!(tmp.path().join("limedl.2.log").exists());
    assert!(!tmp.path().join("limedl.3.log").exists());
    assert!(!tmp.path().join("limedl.4.log").exists());

    // Documented meaning of `Some(0)`: delete every old log.
    cleanup_by_count(&current, 0);
    assert!(!tmp.path().join("limedl.1.log").exists());
    assert!(!tmp.path().join("limedl.2.log").exists());
}

#[test]
fn cleanup_by_age_removes_only_old_files() {
    let tmp = tempdir().expect("tempdir");
    let current = tmp.path().join("limedl.log");
    let old_rotated = tmp.path().join("limedl.1.log");
    let fresh_rotated = tmp.path().join("limedl.2.log");
    write_file(&current, "current");
    write_file(&old_rotated, "old");
    write_file(&fresh_rotated, "fresh");
    age_file(&old_rotated, Duration::from_secs(10 * 86400));
    // The current file is checked too: it may be stale after logging was off.
    age_file(&current, Duration::from_secs(10 * 86400));

    cleanup_by_age(&current, 7);

    assert!(!old_rotated.exists(), "an old rotated file must be removed");
    assert!(
        !current.exists(),
        "an old current file must be removed as well"
    );
    assert_eq!(
        fs::read_to_string(&fresh_rotated).unwrap(),
        "fresh",
        "files inside the window must survive"
    );
}

/// Startup rotation is skipped while another process holds the lock, and
/// runs as soon as it is released.
#[test]
fn perform_startup_rotation_skips_while_lock_is_held() {
    let tmp = tempdir().expect("tempdir");
    let current = tmp.path().join("limedl.log");
    write_file(&current, "current");

    let lock_path = tmp.path().join(".lock");
    let held = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .expect("open lock file");
    held.try_lock().expect("test must acquire the lock");

    perform_startup_rotation(&current, None, None);
    assert!(
        current.exists(),
        "rotation must be skipped while the lock is held"
    );
    assert!(!tmp.path().join("limedl.1.log").exists());

    drop(held);
    perform_startup_rotation(&current, Some(1), None);
    assert!(!current.exists(), "rotation runs once the lock is free");
    assert_eq!(
        fs::read_to_string(tmp.path().join("limedl.1.log")).unwrap(),
        "current"
    );
}

#[test]
fn dynamic_file_writer_honors_enabled_and_survives_open_failure() {
    let tmp = tempdir().expect("tempdir");
    let runtime = |enabled, path: PathBuf| DynamicFileWriter {
        runtime: Arc::new(Mutex::new(LoggerRuntime::new(enabled, path, None, None))),
    };

    // Disabled: writes are accepted and discarded, no file appears.
    let disabled_path = tmp.path().join("disabled.log");
    runtime(false, disabled_path.clone())
        .make_writer()
        .write_all(b"dropped")
        .expect("a disabled writer must accept writes");
    assert!(!disabled_path.exists());

    // Enabled: the parent chain is created and the bytes reach the file.
    let nested = tmp.path().join("nested").join("logs").join("limedl.log");
    runtime(true, nested.clone())
        .make_writer()
        .write_all(b"kept")
        .expect("an enabled writer must accept writes");
    assert_eq!(fs::read_to_string(&nested).unwrap(), "kept");

    // Open failure (the path points at a directory) degrades to a sink —
    // the error is reported on stderr, never as a panic in a tracing call.
    let as_directory = tmp.path().join("as-directory.log");
    fs::create_dir(&as_directory).unwrap();
    runtime(true, as_directory.clone())
        .make_writer()
        .write_all(b"discarded")
        .expect("an unopenable log file must degrade to a sink");
    assert!(as_directory.is_dir());
}

#[test]
fn dynamic_file_writer_rotates_in_place_when_exceeding_size_threshold() {
    let tmp = tempdir().expect("tempdir");
    let log_path = tmp.path().join("limedl.log");
    let mut rt = LoggerRuntime::new(true, log_path.clone(), Some(2), None);
    rt.max_file_bytes = 16;
    let writer = DynamicFileWriter {
        runtime: Arc::new(Mutex::new(rt)),
    };

    writer
        .make_writer()
        .write_all(b"0123456789abcdef")
        .expect("write 16 bytes triggers rotation");
    writer
        .make_writer()
        .write_all(b"after")
        .expect("write after rotation");

    assert_eq!(
        fs::read_to_string(tmp.path().join("limedl.1.log")).unwrap(),
        "0123456789abcdef"
    );
    assert_eq!(fs::read_to_string(&log_path).unwrap(), "after");
}
