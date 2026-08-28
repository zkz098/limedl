use std::fs;
use std::sync::Arc;

use bytes::Bytes;
use tempfile::tempdir;

use super::*;
use crate::buffer_pool::SyncMode;

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
