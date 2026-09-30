//! HDD double-buffer flush paths.

use super::*;

#[tokio::test]
#[timeout(10000)]
async fn test_hdd_buffer_creation() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
    let (_dir, file) = temp_file();
    let slot = pool.acquire_slot().await;
    let buf = DownloadBuffer::new(pool.clone(), slot, file);

    assert_eq!(buf.len(), 0);
    assert!(!buf.has_degraded());
    assert_eq!(pool.active_slots(), 1);
}

#[tokio::test]
#[timeout(30000)]
async fn test_hdd_buffer_chunk_small_and_flush() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
    let (_dir, file) = temp_file();
    let slot = pool.acquire_slot().await;
    let buf = DownloadBuffer::new(pool.clone(), slot, file);

    let data = Bytes::from("hello world");
    buf.buffer_chunk(0, data.clone()).await.unwrap();
    assert_eq!(buf.len(), 11);

    buf.flush_all().await.unwrap();
    assert_eq!(buf.len(), 0);
    let content = fs::read(_dir.path().join("test.bin")).unwrap();
    assert_eq!(&content[..11], b"hello world");
}

#[tokio::test]
#[timeout(30000)]
async fn test_hdd_buffer_multiple_chunks_and_flush() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
    let (_dir, file) = temp_file();
    let slot = pool.acquire_slot().await;
    let buf = DownloadBuffer::new(pool.clone(), slot, file);

    buf.buffer_chunk(0, Bytes::from("AAA")).await.unwrap();
    buf.buffer_chunk(3, Bytes::from("BBB")).await.unwrap();
    buf.buffer_chunk(6, Bytes::from("CCC")).await.unwrap();
    assert_eq!(buf.len(), 9);

    buf.flush_all().await.unwrap();
    let content = fs::read(_dir.path().join("test.bin")).unwrap();
    assert_eq!(&content[..9], b"AAABBBCCC");
}

#[tokio::test]
#[timeout(30000)]
async fn test_hdd_buffer_chunk_triggers_flip_and_flush() {
    let pool = Arc::new(BufferPool::new(4, 128, 2, 1));
    let half = pool.half_size();
    assert!(half >= 64 * KB);

    let (_dir, file) = temp_file();
    let slot = pool.acquire_slot().await;
    let buf = DownloadBuffer::new(pool.clone(), slot, file);

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
    assert!(
        content.len() >= total_written as usize,
        "expected at least {} bytes, got {}",
        total_written,
        content.len()
    );

    for i in 0..3u64 {
        let off = (i * chunk_size) as usize;
        let expected_byte = i as u8;
        assert_eq!(
            content[off], expected_byte,
            "byte at offset {off} should be {expected_byte}"
        );
    }
}

#[tokio::test]
#[timeout(30000)]
async fn test_hdd_buffer_large_chunk_direct_write() {
    let pool = Arc::new(BufferPool::new(4, 128, 2, 1));
    let half = pool.half_size();
    let (_dir, file) = temp_file();
    let slot = pool.acquire_slot().await;
    let buf = DownloadBuffer::new(pool.clone(), slot, file);

    let big_data = vec![0xABu8; (half + 1) as usize];
    let big = Bytes::from(big_data);
    buf.buffer_chunk(0, big.clone()).await.unwrap();

    assert_eq!(buf.len(), 0);

    let content = fs::read(_dir.path().join("test.bin")).unwrap();
    assert_eq!(&content[..(half + 1) as usize], &big[..]);
}

#[tokio::test]
#[timeout(10000)]
async fn test_flush_all_hdd_empty() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
    let (_dir, file) = temp_file();
    let slot = pool.acquire_slot().await;
    let buf = DownloadBuffer::new(pool.clone(), slot, file);

    buf.flush_all().await.unwrap();
}

#[tokio::test]
#[timeout(30000)]
async fn test_clear_hdd() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
    let (_dir, file) = temp_file();
    let slot = pool.acquire_slot().await;
    let buf = DownloadBuffer::new(pool.clone(), slot, file);

    buf.buffer_chunk(0, Bytes::from("discard me"))
        .await
        .unwrap();
    assert_eq!(buf.len(), 10);

    buf.clear();
    assert_eq!(buf.len(), 0);

    buf.buffer_chunk(0, Bytes::from("new data")).await.unwrap();
    assert_eq!(buf.len(), 8);

    buf.flush_all().await.unwrap();
    let content = fs::read(_dir.path().join("test.bin")).unwrap();
    assert_eq!(&content[..8], b"new data");
}

#[tokio::test]
#[timeout(30000)]
async fn test_drain_background_hdd() {
    let pool = Arc::new(BufferPool::new(8, 128, 2, 1));
    let half = pool.half_size();
    let (_dir, file) = temp_file();
    let slot = pool.acquire_slot().await;
    let buf = DownloadBuffer::new(pool.clone(), slot, file);

    let chunk = vec![0xDDu8; (half + 1) as usize];
    buf.buffer_chunk(0, Bytes::from(chunk)).await.unwrap();
    drop(buf);

    let slot = pool.acquire_slot().await;
    let (_dir2, file2) = temp_file();
    let buf2 = DownloadBuffer::new(pool.clone(), slot, file2);

    let small = half / 4;
    for i in 0..5u64 {
        let payload = vec![i as u8; small as usize];
        buf2.buffer_chunk(i * small, Bytes::from(payload))
            .await
            .unwrap();
    }

    buf2.drain_background().await;
    buf2.flush_all().await.unwrap();
    let content = fs::read(_dir2.path().join("test.bin")).unwrap();
    assert!(content.len() >= (5 * small) as usize);
}

#[tokio::test]
#[timeout(10000)]
async fn test_zero_size_chunk_hdd() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
    let (_dir, file) = temp_file();
    let slot = pool.acquire_slot().await;
    let buf = DownloadBuffer::new(pool.clone(), slot, file);

    buf.buffer_chunk(0, Bytes::new()).await.unwrap();
    assert_eq!(buf.len(), 0);

    buf.flush_all().await.unwrap();
    let content = fs::read(_dir.path().join("test.bin")).unwrap();
    assert!(content.is_empty());
}

#[tokio::test]
#[timeout(30000)]
async fn test_concurrent_hdd_buffer_chunks() {
    let pool = Arc::new(BufferPool::new(32, 128, 4, 1));
    let (_dir, file) = temp_file();
    let file_arc = file.clone();

    let slot = pool.acquire_slot().await;
    let buf = Arc::new(DownloadBuffer::new(pool.clone(), slot, file_arc));

    let mut handles = Vec::new();
    let chunk_size = 4096u64;
    let num_chunks = 4u64;
    for i in 0..num_chunks {
        let b = buf.clone();
        handles.push(tokio::spawn(async move {
            let payload = vec![i as u8; chunk_size as usize];
            b.buffer_chunk(i * chunk_size, Bytes::from(payload))
                .await
                .unwrap();
        }));
    }
    for h in handles {
        h.await.unwrap();
    }

    assert_eq!(buf.len(), num_chunks * chunk_size);

    buf.flush_all().await.unwrap();
    let content = fs::read(_dir.path().join("test.bin")).unwrap();
    assert!(content.len() >= (num_chunks * chunk_size) as usize);

    for i in 0..num_chunks {
        let off = (i * chunk_size) as usize;
        assert_eq!(content[off], i as u8);
    }
}

#[tokio::test]
#[timeout(30000)]
async fn test_overlapping_writes_hdd() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
    let (_dir, file) = temp_file();
    let slot = pool.acquire_slot().await;
    let buf = DownloadBuffer::new(pool.clone(), slot, file);

    buf.buffer_chunk(0, Bytes::from("AAA")).await.unwrap();
    buf.buffer_chunk(0, Bytes::from("BBB")).await.unwrap();

    buf.flush_all().await.unwrap();
    let content = fs::read(_dir.path().join("test.bin")).unwrap();
    assert_eq!(&content[..3], b"BBB");
}

#[tokio::test]
#[timeout(30000)]
async fn test_flush_all_multiple_times_hdd() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
    let (_dir, file) = temp_file();
    let slot = pool.acquire_slot().await;
    let buf = DownloadBuffer::new(pool.clone(), slot, file);

    buf.buffer_chunk(0, Bytes::from("a")).await.unwrap();
    buf.flush_all().await.unwrap();
    assert_eq!(buf.len(), 0);

    buf.buffer_chunk(5, Bytes::from("b")).await.unwrap();
    buf.flush_all().await.unwrap();
    assert_eq!(buf.len(), 0);

    let content = fs::read(_dir.path().join("test.bin")).unwrap();
    assert_eq!(content[0], b'a');
    assert_eq!(content[5], b'b');
}

#[tokio::test]
#[timeout(30000)]
async fn test_buffer_after_flush_hdd() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
    let (_dir, file) = temp_file();
    let slot = pool.acquire_slot().await;
    let buf = DownloadBuffer::new(pool.clone(), slot, file);

    buf.buffer_chunk(0, Bytes::from("first")).await.unwrap();
    buf.flush_all().await.unwrap();
    assert_eq!(buf.len(), 0);

    buf.buffer_chunk(10, Bytes::from("second")).await.unwrap();
    assert_eq!(buf.len(), 6);

    buf.flush_all().await.unwrap();
    let content = fs::read(_dir.path().join("test.bin")).unwrap();
    assert_eq!(&content[0..5], b"first");
    assert_eq!(&content[10..16], b"second");
}

#[tokio::test]
#[timeout(30000)]
async fn test_hdd_with_worker_buffer_chunk_and_flush() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
    let (_dir, file) = temp_file();
    let slot = pool.acquire_slot().await;
    let worker = IoWorker::spawn();
    let buf = DownloadBuffer::new_with_worker(pool.clone(), slot, file.clone(), worker);

    let data = Bytes::from("hello io worker");
    buf.buffer_chunk(0, data.clone()).await.unwrap();
    assert_eq!(buf.len(), 15);

    buf.flush_all().await.unwrap();
    assert_eq!(buf.len(), 0);
    let content = fs::read(_dir.path().join("test.bin")).unwrap();
    assert_eq!(&content[..15], b"hello io worker");
}

#[tokio::test]
#[timeout(30000)]
async fn test_hdd_with_worker_multiple_chunks() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
    let (_dir, file) = temp_file();
    let slot = pool.acquire_slot().await;
    let worker = IoWorker::spawn();
    let buf = DownloadBuffer::new_with_worker(pool.clone(), slot, file.clone(), worker);

    buf.buffer_chunk(0, Bytes::from("AAA")).await.unwrap();
    buf.buffer_chunk(3, Bytes::from("BBB")).await.unwrap();
    buf.buffer_chunk(6, Bytes::from("CCC")).await.unwrap();
    assert_eq!(buf.len(), 9);

    buf.flush_all().await.unwrap();
    let content = fs::read(_dir.path().join("test.bin")).unwrap();
    assert_eq!(&content[..9], b"AAABBBCCC");
}

#[tokio::test]
#[timeout(30000)]
async fn test_background_flush_failure_using_readonly_file() {
    let pool = Arc::new(BufferPool::new(1, 1, 4, 1));
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.bin");

    fs::File::create(&path).expect("create file");
    {
        let mut perms = fs::metadata(&path).expect("metadata").permissions();
        perms.set_readonly(true);
        fs::set_permissions(&path, perms).expect("set read-only");
    }

    let file = Arc::new(
        fs::OpenOptions::new()
            .read(true)
            .open(&path)
            .expect("open read-only file"),
    );

    let slot = pool.acquire_slot().await;
    let buf = DownloadBuffer::new(pool.clone(), slot, file);

    assert!(!buf.has_degraded());
    assert_eq!(buf.len(), 0);

    let half = pool.half_size();
    let chunk_size = half / 2 + 1;
    let chunk1 = Bytes::from(vec![0xABu8; chunk_size as usize]);
    let chunk2 = Bytes::from(vec![0xBCu8; chunk_size as usize]);

    buf.buffer_chunk(0, chunk1).await.unwrap();
    buf.buffer_chunk(half, chunk2).await.unwrap();

    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(5);
    while !buf.has_degraded() {
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        if tokio::time::Instant::now() >= deadline {
            panic!("background flush did not set error flag within 5 s");
        }
    }

    assert!(buf.has_degraded());

    let result = buf.buffer_chunk(half * 2, Bytes::from("fail")).await;
    match result {
        Err(DownloadError::Internal(msg)) => {
            assert!(
                msg.contains("background buffer flush failed"),
                "unexpected message: {msg}"
            );
        }
        other => panic!("expected Err(Internal), got {other:?}"),
    }

    assert!(buf.has_degraded());

    let result = buf.flush_all().await;
    match result {
        Err(DownloadError::Internal(msg)) => {
            assert!(
                msg.contains("background buffer flush failed"),
                "unexpected message: {msg}"
            );
        }
        other => panic!("expected Err(Internal), got {other:?}"),
    }

    drop(buf);
    assert_eq!(pool.active_slots(), 0);
}

#[tokio::test]
#[timeout(30000)]
async fn test_background_flush_failure_using_readonly_file_with_worker() {
    let pool = Arc::new(BufferPool::new(1, 1, 4, 1));
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.bin");

    fs::File::create(&path).expect("create file");
    {
        let mut perms = fs::metadata(&path).expect("metadata").permissions();
        perms.set_readonly(true);
        fs::set_permissions(&path, perms).expect("set read-only");
    }

    let file = Arc::new(
        fs::OpenOptions::new()
            .read(true)
            .open(&path)
            .expect("open read-only file"),
    );

    let slot = pool.acquire_slot().await;
    let worker = IoWorker::spawn();
    let buf = DownloadBuffer::new_with_worker(pool.clone(), slot, file, worker);

    assert!(!buf.has_degraded());
    assert_eq!(buf.len(), 0);

    let half = pool.half_size();
    let chunk_size = half / 2 + 1;
    let chunk1 = Bytes::from(vec![0xABu8; chunk_size as usize]);
    let chunk2 = Bytes::from(vec![0xBCu8; chunk_size as usize]);

    buf.buffer_chunk(0, chunk1).await.unwrap();
    buf.buffer_chunk(half, chunk2).await.unwrap();

    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(5);
    while !buf.has_degraded() {
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        if tokio::time::Instant::now() >= deadline {
            panic!("background flush did not set error flag within 5 s");
        }
    }

    assert!(buf.has_degraded());

    let result = buf.buffer_chunk(half * 2, Bytes::from("fail")).await;
    match result {
        Err(DownloadError::Internal(msg)) => {
            assert!(
                msg.contains("background buffer flush failed"),
                "unexpected message: {msg}"
            );
        }
        other => panic!("expected Err(Internal), got {other:?}"),
    }

    drop(buf);
    assert_eq!(pool.active_slots(), 0);
}
