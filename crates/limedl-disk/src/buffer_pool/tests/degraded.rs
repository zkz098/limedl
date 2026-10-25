//! Degradation flags and error reporting.

use super::*;

#[tokio::test]
#[timeout(10000)]
async fn test_hdd_buffer_degraded_after_clear() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
    let (_dir, file) = temp_file();
    let slot = pool.acquire_slot().await;
    let buf = DownloadBuffer::new(pool.clone(), slot, file);

    assert!(!buf.has_degraded());

    buf.buffer_chunk(0, Bytes::from("data")).await.unwrap();
    buf.clear();
    assert!(!buf.has_degraded());
    assert_eq!(buf.len(), 0);
}

#[tokio::test]
#[timeout(10000)]
async fn test_degraded_flag_detected_by_has_degraded() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
    let (_dir, file) = temp_file();
    let slot = pool.acquire_slot().await;
    let buf = DownloadBuffer::new(pool.clone(), slot, file);

    assert!(!buf.has_degraded());

    if let BufferMode::Double { error_flag, .. } = &buf.mode {
        error_flag.store(true, Ordering::Release);
    }

    assert!(buf.has_degraded());
}

#[tokio::test]
#[timeout(10000)]
async fn test_buffer_chunk_returns_error_when_degraded() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
    let (_dir, file) = temp_file();
    let slot = pool.acquire_slot().await;
    let buf = DownloadBuffer::new(pool.clone(), slot, file);

    buf.buffer_chunk(0, Bytes::from("normal")).await.unwrap();

    if let BufferMode::Double { error_flag, .. } = &buf.mode {
        error_flag.store(true, Ordering::Release);
    }

    let result = buf.buffer_chunk(100, Bytes::from("fail")).await;
    match result {
        Err(DownloadError::Internal(msg)) => {
            assert!(
                msg.contains("background buffer flush failed"),
                "unexpected message: {msg}"
            );
        }
        other => panic!("expected Err(Internal), got {other:?}"),
    }
}

#[tokio::test]
#[timeout(10000)]
async fn test_clear_resets_degraded_flag() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
    let (_dir, file) = temp_file();
    let slot = pool.acquire_slot().await;
    let buf = DownloadBuffer::new(pool.clone(), slot, file);

    if let BufferMode::Double { error_flag, .. } = &buf.mode {
        error_flag.store(true, Ordering::Release);
    }
    assert!(buf.has_degraded());

    buf.clear();
    assert!(!buf.has_degraded());
    assert_eq!(buf.len(), 0);
}

#[tokio::test]
#[timeout(10000)]
async fn test_flush_all_checks_degraded() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
    let (_dir, file) = temp_file();
    let slot = pool.acquire_slot().await;
    let buf = DownloadBuffer::new(pool.clone(), slot, file);

    buf.flush_all().await.unwrap();

    if let BufferMode::Double { error_flag, .. } = &buf.mode {
        error_flag.store(true, Ordering::Release);
    }

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
}

#[tokio::test]
#[timeout(10000)]
async fn test_degradation_count_always_zero() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
    let (_dir, file) = temp_file();
    let slot = pool.acquire_slot().await;
    let buf = DownloadBuffer::new(pool.clone(), slot, file);

    assert_eq!(pool.degradation_count(), 0);

    if let BufferMode::Double { error_flag, .. } = &buf.mode {
        error_flag.store(true, Ordering::Release);
    }
    assert!(buf.has_degraded());

    assert_eq!(pool.degradation_count(), 0);
}

#[tokio::test]
#[timeout(10000)]
async fn test_degraded_flag_persists_after_chunk_failure() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
    let (_dir, file) = temp_file();
    let slot = pool.acquire_slot().await;
    let buf = DownloadBuffer::new(pool.clone(), slot, file);

    if let BufferMode::Double { error_flag, .. } = &buf.mode {
        error_flag.store(true, Ordering::Release);
    }

    assert!(buf.buffer_chunk(0, Bytes::from("data")).await.is_err());
    assert!(buf.has_degraded());
}

#[tokio::test]
#[timeout(10000)]
async fn test_drop_clears_degraded_flag() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
    let (_dir, file) = temp_file();
    let slot = pool.acquire_slot().await;
    let buf = DownloadBuffer::new(pool.clone(), slot, file);

    let error_flag = match &buf.mode {
        BufferMode::Double { error_flag, .. } => error_flag.clone(),
        _ => unreachable!(),
    };

    error_flag.store(true, Ordering::Release);
    assert!(buf.has_degraded());

    drop(buf);
    assert!(!error_flag.load(Ordering::Relaxed));
}
