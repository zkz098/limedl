use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use crate::file_ops::{is_network_filesystem, parse_mountinfo, path_is_within};
use crate::types::DiskType;

pub fn detect_disk_type(path: &Path) -> DiskType {
    // Network and host-brokered mounts have no local block device to
    // inspect, so every check below would answer "SSD" for an NFS share —
    // and for WSL's `/mnt/c`, which is a 9p transport over a VHDX whose
    // media the guest cannot see. Classify those first.
    if let Some(media) = network_mount_media(path) {
        return media;
    }

    let Ok(meta) = fs::metadata(path) else {
        return DiskType::Ssd;
    };
    use std::os::linux::fs::MetadataExt;
    let dev = meta.st_dev();
    let major = rustix::fs::major(dev);
    let minor = rustix::fs::minor(dev);

    let dev_symlink = format!("/sys/dev/block/{major}:{minor}");
    let Ok(link) = fs::read_link(&dev_symlink) else {
        return DiskType::Ssd;
    };

    // Use file_name() directly — handles both partition (sda1) and
    // whole-disk (sda) cases, since modern Linux creates sysfs entries
    // for partition devices under /sys/block/ as well.
    let Some(device_name) = link.file_name().and_then(OsStr::to_str) else {
        return DiskType::Ssd;
    };

    let rotational_path = format!("/sys/block/{device_name}/queue/rotational");
    match fs::read_to_string(&rotational_path) {
        Ok(val) if val.trim() == "1" => DiskType::Hdd,
        _ => DiskType::Ssd,
    }
}

use std::collections::HashMap;

/// Media for `path` when it lives on a network or host-brokered mount.
fn network_mount_media(path: &Path) -> Option<DiskType> {
    let absolute = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let mountinfo = fs::read_to_string("/proc/self/mountinfo").ok()?;

    let mut best: Option<usize> = None;
    let mut is_network = false;
    for (mount_point, fstype) in parse_mountinfo(&mountinfo) {
        if !path_is_within(&absolute, Path::new(&mount_point)) {
            continue;
        }
        // The longest matching mount point wins: `/mnt/nas/sub` is a
        // different filesystem from `/mnt`.
        if best.is_none_or(|length| mount_point.len() > length) {
            best = Some(mount_point.len());
            is_network = is_network_filesystem(&fstype);
        }
    }

    (best.is_some() && is_network).then_some(DiskType::Network)
}

pub fn detect_all_disk_types() -> HashMap<String, DiskType> {
    let mut result = HashMap::new();
    let Ok(entries) = fs::read_dir("/sys/block") else {
        return result;
    };
    for entry in entries.flatten() {
        let rotational_path = entry.path().join("queue/rotational");
        if let Ok(val) = fs::read_to_string(&rotational_path) {
            let name = entry.file_name().to_string_lossy().to_string();
            let disk_type = if val.trim() == "1" {
                DiskType::Hdd
            } else {
                DiskType::Ssd
            };
            result.insert(name, disk_type);
        }
    }
    // Block devices cannot describe a network mount, but they are exactly
    // the locations a user needs to be able to name before pinning one with
    // a media override.
    if let Ok(mountinfo) = fs::read_to_string("/proc/self/mountinfo") {
        for (mount_point, fstype) in parse_mountinfo(&mountinfo) {
            if is_network_filesystem(&fstype) {
                result.insert(mount_point, DiskType::Network);
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mountinfo parser and the component-boundary match are tested in
    /// `file_ops::media` — they are platform-independent, and that is where
    /// the Windows gate can actually compile them. This pins the Linux
    /// wiring only: a local path must not be classified as remote, and a
    /// local filesystem must still be probed from its block device.
    #[test]
    fn local_paths_are_not_network_mounts() {
        assert_eq!(network_mount_media(Path::new("/tmp")), None);
        assert_ne!(detect_disk_type(Path::new("/")), DiskType::Network);
    }
}
