//! DownloadSlotGuard acquire/release semantics.

use super::*;

#[tokio::test]
#[timeout(10000)]
async fn test_release_slot_direct() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
    let _guard = pool.acquire_slot().await;
    assert_eq!(pool.active_slots(), 1);
    pool.release_slot();
    assert_eq!(pool.active_slots(), 0);
}

#[tokio::test]
#[timeout(10000)]
async fn test_slot_guard_drop_releases_semaphore_permit() {
    let pool = Arc::new(BufferPool::new(1024, 128, 1, 1));

    let guard = pool.acquire_slot().await;
    assert_eq!(pool.slot_semaphore.available_permits(), 0);

    drop(guard);
    assert_eq!(pool.slot_semaphore.available_permits(), 1);
}

#[tokio::test]
#[timeout(10000)]
async fn test_slot_guard_drop_allows_another_acquire() {
    let pool = Arc::new(BufferPool::new(1024, 128, 1, 1));

    let guard = pool.acquire_slot().await;
    drop(guard);

    let _guard2 = pool.acquire_slot().await;
    assert_eq!(pool.active_slots(), 2);

    pool.release_slot();
    pool.release_slot();
    assert_eq!(pool.active_slots(), 0);
}

#[tokio::test]
#[timeout(10000)]
async fn slot_guard_drop_releases_permit() {
    let pool = Arc::new(BufferPool::new(1, 64, 1, 64));

    let guard = pool.acquire_slot().await;
    assert_eq!(pool.active_slots(), 1);
    assert_eq!(pool.slot_semaphore.available_permits(), 0);

    drop(guard);
    assert_eq!(pool.slot_semaphore.available_permits(), 1);
    assert_eq!(pool.active_slots(), 1);

    pool.release_slot();
    assert_eq!(pool.active_slots(), 0);
}

#[tokio::test]
#[timeout(10000)]
async fn slot_guard_semaphore_limits_concurrency() {
    let pool = Arc::new(BufferPool::new(1, 64, 1, 64));

    let guard1 = pool.acquire_slot().await;
    assert_eq!(pool.slot_semaphore.available_permits(), 0);

    let pool2 = pool.clone();
    let acquired = Arc::new(AtomicBool::new(false));
    let acquired2 = acquired.clone();

    let handle = tokio::spawn(async move {
        let _g2 = pool2.acquire_slot().await;
        acquired2.store(true, Ordering::Relaxed);
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    assert!(
        !acquired.load(Ordering::Relaxed),
        "second acquire should block when semaphore is exhausted"
    );

    drop(guard1);

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    assert!(
        acquired.load(Ordering::Relaxed),
        "second acquire should succeed after first guard is dropped"
    );

    handle.await.unwrap();

    pool.release_slot();
    pool.release_slot();
    assert_eq!(pool.active_slots(), 0);
}

#[tokio::test]
#[timeout(10000)]
async fn test_download_buffer_drop_releases_slot() {
    let pool = Arc::new(BufferPool::new(1024, 128, 2, 1));
    let (_dir, file) = temp_file();

    let slot = pool.acquire_slot().await;
    assert_eq!(pool.active_slots(), 1);

    {
        let _buf = DownloadBuffer::new(pool.clone(), slot, file);
        assert_eq!(pool.active_slots(), 1);
    }
    assert_eq!(
        pool.active_slots(),
        0,
        "DownloadBuffer::drop should release the slot"
    );
}

#[tokio::test]
#[timeout(10000)]
async fn test_download_buffer_drop_clears_usage() {
    let pool = Arc::new(BufferPool::new(1024, 128, 2, 1));
    let (_dir, file) = temp_file();

    let slot = pool.acquire_slot().await;
    {
        let buf = DownloadBuffer::new(pool.clone(), slot, file);
        buf.buffer_chunk(0, Bytes::from("data")).await.unwrap();
        assert!(pool.current_usage() > 0);
    }
    assert_eq!(pool.current_usage(), 0);
}
