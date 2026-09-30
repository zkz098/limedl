//! SSD ping-pong buffer paths.

use super::*;

#[tokio::test]
#[timeout(30000)]
async fn test_ssd_pingpong_buffer_creation() {
    let (_dir, file) = temp_file();
    let worker = IoWorker::spawn();
    let buf = DownloadBuffer::new_local_pingpong_with_worker(64 * 1024, file.clone(), worker);
    assert_eq!(buf.len(), 0);
    assert!(!buf.has_degraded());
}

#[tokio::test]
#[timeout(30000)]
async fn test_ssd_pingpong_buffer_chunk_and_flush() {
    let (_dir, file) = temp_file();
    let worker = IoWorker::spawn();
    let buf = DownloadBuffer::new_local_pingpong_with_worker(4 * MB, file.clone(), worker);

    let data = Bytes::from("hello pingpong ssd");
    buf.buffer_chunk(0, data.clone()).await.unwrap();
    assert_eq!(buf.len(), 18);

    buf.flush_all().await.unwrap();
    assert_eq!(buf.len(), 0);
    let content = fs::read(_dir.path().join("test.bin")).unwrap();
    assert_eq!(&content[..18], b"hello pingpong ssd");
}

#[tokio::test]
#[timeout(30000)]
async fn test_ssd_pingpong_multiple_offsets() {
    let (_dir, file) = temp_file();
    let worker = IoWorker::spawn();
    let buf = DownloadBuffer::new_local_pingpong_with_worker(4 * MB, file.clone(), worker);

    buf.buffer_chunk(0, Bytes::from("aaaa")).await.unwrap();
    buf.buffer_chunk(10, Bytes::from("bbbb")).await.unwrap();
    buf.buffer_chunk(20, Bytes::from("cccc")).await.unwrap();
    assert_eq!(buf.len(), 12);

    buf.flush_all().await.unwrap();
    assert_eq!(buf.len(), 0);

    let content = fs::read(_dir.path().join("test.bin")).unwrap();
    assert_eq!(&content[0..4], b"aaaa");
    assert_eq!(&content[10..14], b"bbbb");
    assert_eq!(&content[20..24], b"cccc");
}

#[tokio::test]
#[timeout(30000)]
async fn test_ssd_pingpong_flip_trigger() {
    let half = 1024u64;
    let (_dir, file) = temp_file();
    let worker = IoWorker::spawn();
    let buf = DownloadBuffer::new_local_pingpong_with_worker(half, file.clone(), worker);

    let chunk_size = half / 2;
    let mut total_written = 0u64;
    for i in 0..3u64 {
        let payload = vec![i as u8; chunk_size as usize];
        buf.buffer_chunk(i * chunk_size, Bytes::from(payload))
            .await
            .unwrap();
        total_written += chunk_size;
    }
    assert!(!buf.is_empty());

    buf.flush_all().await.unwrap();
    assert_eq!(buf.len(), 0);

    let content = fs::read(_dir.path().join("test.bin")).unwrap();
    assert!(content.len() >= total_written as usize);
    for i in 0..3u64 {
        let off = (i * chunk_size) as usize;
        assert_eq!(content[off], i as u8);
    }
}

#[tokio::test]
#[timeout(30000)]
async fn test_ssd_pingpong_drain_and_clear() {
    let half = 1024u64;
    let (_dir, file) = temp_file();
    let worker = IoWorker::spawn();
    let buf = DownloadBuffer::new_local_pingpong_with_worker(half, file.clone(), worker);

    let small = half / 4;
    for i in 0..5u64 {
        let payload = vec![i as u8; small as usize];
        buf.buffer_chunk(i * small, Bytes::from(payload))
            .await
            .unwrap();
    }
    buf.drain_background().await;
    buf.flush_all().await.unwrap();

    let content = fs::read(_dir.path().join("test.bin")).unwrap();
    assert!(content.len() >= (5 * small) as usize);

    buf.clear();
    assert_eq!(buf.len(), 0);
    assert!(!buf.has_degraded());
}

#[tokio::test]
#[timeout(10000)]
async fn test_ssd_pingpong_error_flag() {
    let (_dir, file) = temp_file();
    let worker = IoWorker::spawn();
    let buf = DownloadBuffer::new_local_pingpong_with_worker(4 * MB, file.clone(), worker);

    assert!(!buf.has_degraded());

    if let BufferMode::LocalPingPong { error_flag, .. } = &buf.mode {
        error_flag.store(true, Ordering::Release);
    }

    assert!(buf.has_degraded());

    let result = buf.buffer_chunk(100, Bytes::from("fail")).await;
    assert!(result.is_err());

    buf.clear();
    assert!(!buf.has_degraded());
}

#[tokio::test]
#[timeout(10000)]
async fn test_ssd_pingpong_flush_all_empty() {
    let (_dir, file) = temp_file();
    let worker = IoWorker::spawn();
    let buf = DownloadBuffer::new_local_pingpong_with_worker(4 * MB, file.clone(), worker);
    buf.flush_all().await.unwrap();
}

#[tokio::test]
#[timeout(30000)]
async fn test_ssd_pingpong_overlapping_writes() {
    let (_dir, file) = temp_file();
    let worker = IoWorker::spawn();
    let buf = DownloadBuffer::new_local_pingpong_with_worker(4 * MB, file.clone(), worker);

    buf.buffer_chunk(0, Bytes::from("XXX")).await.unwrap();
    buf.buffer_chunk(0, Bytes::from("YYY")).await.unwrap();

    buf.flush_all().await.unwrap();
    let content = fs::read(_dir.path().join("test.bin")).unwrap();
    assert_eq!(&content[..3], b"YYY");
}

#[tokio::test]
#[timeout(30000)]
async fn test_ssd_pingpong_flush_all_multiple_times() {
    let (_dir, file) = temp_file();
    let worker = IoWorker::spawn();
    let buf = DownloadBuffer::new_local_pingpong_with_worker(4 * MB, file.clone(), worker);

    buf.buffer_chunk(0, Bytes::from("first")).await.unwrap();
    buf.flush_all().await.unwrap();
    assert_eq!(buf.len(), 0);

    buf.buffer_chunk(10, Bytes::from("second")).await.unwrap();
    buf.flush_all().await.unwrap();
    assert_eq!(buf.len(), 0);

    let content = fs::read(_dir.path().join("test.bin")).unwrap();
    assert_eq!(&content[0..5], b"first");
    assert_eq!(&content[10..16], b"second");
}

#[tokio::test]
#[timeout(30000)]
async fn test_ssd_pingpong_large_chunk_direct_write() {
    let half = 64 * 1024u64;
    let (_dir, file) = temp_file();
    let worker = IoWorker::spawn();
    let buf = DownloadBuffer::new_local_pingpong_with_worker(half, file.clone(), worker);

    let big_data = vec![0xABu8; (half + 1) as usize];
    let big = Bytes::from(big_data);
    buf.buffer_chunk(0, big.clone()).await.unwrap();

    assert_eq!(buf.len(), 0);

    let content = fs::read(_dir.path().join("test.bin")).unwrap();
    assert_eq!(&content[..(half + 1) as usize], &big[..]);
}
