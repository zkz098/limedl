use std::collections::HashMap;
use std::fs;
use std::sync::Arc;

use bytes::Bytes;
use tempfile::tempdir;

use super::*;
use crate::buffer_pool::SyncMode;

fn override_map(entries: &[(&str, DiskType)]) -> HashMap<String, DiskType> {
    entries
        .iter()
        .map(|(key, media)| (key.to_string(), *media))
        .collect()
}

#[tokio::test]
async fn test_device_topology_and_queue_creation() {
    let manager = DiskDeviceManager::new();
    let temp = tempdir().expect("tempdir");
    let test_path = temp.path().join("test.bin");

    let (dev_id, disk_type) = manager.resolve_device(&test_path);
    assert!(!dev_id.to_string().is_empty());
    println!("Detected device: {dev_id}, disk_type: {disk_type:?}");

    let queue = manager.get_or_create_queue(&test_path);
    let metrics = queue.metrics();
    assert_eq!(metrics.bytes_written, 0);
    assert_eq!(metrics.write_ops_count, 0);
}

#[tokio::test]
async fn test_disk_device_manager_write_batch() {
    let manager = DiskDeviceManager::new();
    let temp = tempdir().expect("tempdir");
    let test_path = temp.path().join("output.dat");

    let file = fs::File::create(&test_path).expect("create file");
    let file = Arc::new(file);

    let entries = vec![
        (0u64, Bytes::from_static(b"LimeDL ")),
        (7u64, Bytes::from_static(b"Global ")),
        (14u64, Bytes::from_static(b"IO Scheduler")),
    ];

    manager
        .write_batch(&test_path, file, entries, SyncMode::None)
        .await
        .expect("write_batch succeeded");

    let content = fs::read(&test_path).expect("read file");
    assert_eq!(content, b"LimeDL Global IO Scheduler");

    let all_metrics = manager.get_device_metrics();
    assert!(!all_metrics.is_empty());
    let total_written: u64 = all_metrics.iter().map(|m| m.bytes_written).sum();
    assert_eq!(total_written, 26);
}

/// The queue records the media it was built for, and that media is what picks
/// the writer-thread count in `DeviceQueue::new` — so this asserts the policy
/// the queue runs under, not just a label.
#[tokio::test]
async fn media_overrides_reach_device_resolution_and_queues() {
    let manager = DiskDeviceManager::new();
    let temp = tempdir().expect("tempdir");
    let dir = temp.path().to_string_lossy().to_string();
    let file_path = temp.path().join("payload.bin");

    // Whatever the local heuristics say — SSD on a CI runner, HDD on a spinning
    // disk — the override has to be the opposite, so the assertions cannot pass
    // by accident on either kind of host.
    let detected = manager.resolve_device(&file_path).1;
    let forced = if detected == DiskType::Hdd {
        DiskType::Ssd
    } else {
        DiskType::Hdd
    };

    let before = manager.get_or_create_queue(&file_path);
    assert_eq!(before.metrics().disk_type, detected);

    manager.set_overrides(&override_map(&[(&dir, forced)]));
    assert_eq!(
        manager.resolve_device(&file_path).1,
        forced,
        "an override must outrank detection"
    );
    // A queue built before the override cannot adopt the new policy, so it is
    // replaced rather than reused (this is what used to require a restart).
    assert_eq!(manager.get_or_create_queue(&file_path).metrics().disk_type, forced);

    // Clearing the overrides restores the detected answer.
    manager.set_overrides(&override_map(&[]));
    assert_eq!(manager.resolve_device(&file_path).1, detected);
    assert_eq!(manager.get_or_create_queue(&file_path).metrics().disk_type, detected);
}

#[tokio::test]
async fn unchanged_overrides_keep_the_existing_device_queues() {
    let manager = DiskDeviceManager::new();
    let temp = tempdir().expect("tempdir");
    let dir = temp.path().to_string_lossy().to_string();

    manager.record_write(1024);
    assert_eq!(manager.get_device_metrics().len(), 1);

    manager.set_overrides(&override_map(&[(&dir, DiskType::Hdd)]));
    assert!(
        manager.get_device_metrics().is_empty(),
        "a changed policy drops the queues, and with them their live metrics"
    );

    manager.record_write(1);
    manager.set_overrides(&override_map(&[(&dir, DiskType::Hdd)]));
    assert_eq!(
        manager.get_device_metrics().len(),
        1,
        "an identical policy must not rebuild anything"
    );
}
