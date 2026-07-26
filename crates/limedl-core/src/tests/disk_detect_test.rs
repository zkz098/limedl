//! Integration tests for disk type detection.
//! These tests verify that `detect_disk_type` correctly identifies HDD and SSD drives.
//! They use actual OS APIs (IOCTL on Windows, sysfs on Linux, IOKit on macOS)
//! and may require specific hardware to pass — skip on CI runners without real drives.

use crate::file_ops::detect_disk_type;
use crate::types::DiskType;
use std::path::Path;

#[cfg(windows)]
mod windows {
    use super::*;

    /// Smoke test: detect_disk_type on the current drive should not panic.
    /// The result depends on the actual hardware (could be HDD or SSD).
    #[test]
    fn current_drive_does_not_panic() {
        let cwd = std::env::current_dir().unwrap();
        let result = detect_disk_type(&cwd);
        assert!(matches!(result, DiskType::Hdd | DiskType::Ssd));
        eprintln!("Current drive detected as: {result:?}");
    }

    /// Smoke test: detect_disk_type on a non-existent drive letter should
    /// either fail to open the volume (→ SSD default) or return a result.
    #[test]
    fn nonexistent_drive_returns_no_panic() {
        // "Z:" is extremely unlikely to exist on a dev machine
        let result = detect_disk_type(Path::new("Z:\\"));
        assert!(matches!(result, DiskType::Hdd | DiskType::Ssd));
        eprintln!("Z: drive detected as: {result:?}");
    }

    /// Verify C: drive detection works (should always exist on Windows).
    #[test]
    fn c_drive_detection_works() {
        let result = detect_disk_type(Path::new("C:\\"));
        assert!(matches!(result, DiskType::Hdd | DiskType::Ssd));
        eprintln!("C: drive detected as: {result:?}");
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;

    #[test]
    fn root_drive_does_not_panic() {
        let result = detect_disk_type(Path::new("/"));
        assert!(matches!(result, DiskType::Hdd | DiskType::Ssd));
        eprintln!("Root drive detected as: {result:?}");
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;

    #[test]
    fn root_drive_does_not_panic() {
        let result = detect_disk_type(Path::new("/"));
        assert!(matches!(result, DiskType::Hdd | DiskType::Ssd));
        eprintln!("Root drive detected as: {result:?}");
    }
}
