use super::download_buffer::{BufferMode, FlipTokenGuard};
use super::*;
use bytes::Bytes;
use ntest::timeout;
use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tempfile::tempdir;

use crate::error::DownloadError;

const KB: u64 = 1024;
const MB: u64 = 1024 * 1024;

/// Create a temporary file wrapped in `Arc<File>` plus the `TempDir` guard.
fn temp_file() -> (tempfile::TempDir, Arc<std::fs::File>) {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.bin");
    let file = fs::File::create(&path).expect("create file");
    (dir, Arc::new(file))
}

// -----------------------------------------------------------------------
// BufferPool construction & defaults
// -----------------------------------------------------------------------

#[tokio::test]
#[timeout(10000)]
async fn test_pool_creation_defaults() {
    let pool = BufferPool::new(1024, 128, 4, 1);
    assert_eq!(pool.effective_limit(), 1024 * MB);
    assert_eq!(pool.effective_max_parallel(), 4);
    assert_eq!(pool.max_slots(), 4);
    assert!(!pool.game_mode());
    assert_eq!(pool.current_usage(), 0);
    assert_eq!(pool.active_slots(), 0);
    assert_eq!(pool.queued_count(), 0);
    assert_eq!(pool.degradation_count(), 0);
}

#[tokio::test]
#[timeout(10000)]
async fn test_pool_creation_custom_limits() {
    let pool = BufferPool::new(512, 64, 8, 2);
    assert_eq!(pool.effective_limit(), 512 * MB);
    assert_eq!(pool.effective_max_parallel(), 8);
    assert_eq!(pool.max_slots(), 8);
    assert!(!pool.game_mode());
}

#[tokio::test]
#[timeout(10000)]
async fn test_pool_creation_game_mode_on_start() {
    let pool = BufferPool::new(1024, 128, 4, 1);
    assert!(!pool.game_mode());
    assert_eq!(pool.effective_limit(), 1024 * MB);
    assert_eq!(pool.effective_max_parallel(), 4);

    pool.set_game_mode(true);
    assert!(pool.game_mode());
    assert_eq!(pool.effective_limit(), 128 * MB);
    assert_eq!(pool.effective_max_parallel(), 1);
}

// -----------------------------------------------------------------------
// half_size()
// -----------------------------------------------------------------------

#[tokio::test]
#[timeout(10000)]
async fn test_half_size_normal() {
    let pool = BufferPool::new(1024, 128, 4, 1);
    let expected = 1024 * MB / 4 / 2;
    assert_eq!(pool.half_size(), expected);
}

#[tokio::test]
#[timeout(10000)]
async fn test_half_size_minimum() {
    let pool = BufferPool::new(0, 0, 4, 1);
    assert_eq!(pool.half_size(), 64 * KB);

    let pool = BufferPool::new(1, 1, 32, 1);
    assert_eq!(pool.half_size(), 64 * KB);
}

#[tokio::test]
#[timeout(10000)]
async fn test_half_size_zero_slots() {
    let pool = BufferPool::new(1024, 128, 0, 0);
    assert_eq!(pool.half_size(), 64 * KB);
}

#[tokio::test]
#[timeout(10000)]
async fn test_half_size_respects_game_mode() {
    let pool = BufferPool::new(1024, 128, 4, 1);
    let normal = pool.half_size();

    pool.set_game_mode(true);
    let expected_game = 128 * MB / 2;
    assert_eq!(pool.half_size(), expected_game.max(64 * KB));
    assert!(
        pool.half_size() < normal,
        "game-mode half_size ({}) should be smaller than normal ({})",
        pool.half_size(),
        normal,
    );

    pool.set_game_mode(false);
    assert_eq!(pool.half_size(), normal);
}

// -----------------------------------------------------------------------
// acquire_slot / release_slot / active_count
// -----------------------------------------------------------------------

#[tokio::test]
#[timeout(10000)]
async fn test_acquire_slot_increments_active_count() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
    assert_eq!(pool.active_slots(), 0);

    let guard = pool.acquire_slot().await;
    assert_eq!(pool.active_slots(), 1);
    drop(guard);
    assert_eq!(pool.active_slots(), 1);

    pool.release_slot();
    assert_eq!(pool.active_slots(), 0);
}

#[tokio::test]
#[timeout(10000)]
async fn test_acquire_multiple_slots_sequential() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));

    let g1 = pool.acquire_slot().await;
    assert_eq!(pool.active_slots(), 1);

    let g2 = pool.acquire_slot().await;
    assert_eq!(pool.active_slots(), 2);

    let g3 = pool.acquire_slot().await;
    assert_eq!(pool.active_slots(), 3);

    let g4 = pool.acquire_slot().await;
    assert_eq!(pool.active_slots(), 4);

    drop(g1);
    drop(g2);
    drop(g3);
    drop(g4);
    assert_eq!(pool.active_slots(), 4);

    pool.release_slot();
    pool.release_slot();
    pool.release_slot();
    pool.release_slot();
    assert_eq!(pool.active_slots(), 0);
}

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
async fn test_acquire_all_slots_and_verify_semaphore_exhausted() {
    let pool = Arc::new(BufferPool::new(1024, 128, 2, 1));
    let _g1 = pool.acquire_slot().await;
    let _g2 = pool.acquire_slot().await;

    assert_eq!(pool.slot_semaphore.available_permits(), 0);
}

#[tokio::test]
#[timeout(10000)]
async fn test_acquire_blocks_when_all_slots_taken() {
    let pool = Arc::new(BufferPool::new(1024, 128, 1, 1));
    let _g1 = pool.acquire_slot().await;
    assert_eq!(pool.active_slots(), 1);

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
        "spawned task should be blocked"
    );

    drop(_g1);

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    assert!(acquired.load(Ordering::Relaxed));
    handle.await.unwrap();
}

#[tokio::test]
#[timeout(10000)]
async fn test_rapid_acquire_release_cycle() {
    let pool = Arc::new(BufferPool::new(1024, 128, 100, 1));
    let mut guards = Vec::new();
    for _ in 0..100 {
        let guard = pool.acquire_slot().await;
        assert_eq!(pool.active_slots(), guards.len() as u32 + 1);
        guards.push(guard);
    }
    assert_eq!(pool.active_slots(), 100);
    for guard in guards {
        drop(guard);
    }
    assert_eq!(pool.active_slots(), 100);
    for _ in 0..100 {
        pool.release_slot();
    }
    assert_eq!(pool.active_slots(), 0);
}

// -----------------------------------------------------------------------
// Memory tracking
// -----------------------------------------------------------------------

#[tokio::test]
#[timeout(10000)]
async fn test_memory_tracking_add_sub_usage() {
    let pool = BufferPool::new(1024, 128, 4, 1);
    assert_eq!(pool.current_usage(), 0);

    pool.add_usage(100);
    assert_eq!(pool.current_usage(), 100);

    pool.add_usage(50);
    assert_eq!(pool.current_usage(), 150);

    pool.sub_usage(30);
    assert_eq!(pool.current_usage(), 120);

    pool.sub_usage(120);
    assert_eq!(pool.current_usage(), 0);
}

#[tokio::test]
#[timeout(10000)]
async fn test_memory_tracking_underflow() {
    let pool = BufferPool::new(1024, 128, 4, 1);
    pool.add_usage(10);
    pool.sub_usage(100);
    let usage = pool.current_usage();
    assert!(
        usage > (u64::MAX - 100),
        "underflow should wrap to a large value, got {usage}"
    );
}

// -----------------------------------------------------------------------
// Game mode
// -----------------------------------------------------------------------

#[tokio::test]
#[timeout(10000)]
async fn test_game_mode_toggle() {
    let pool = BufferPool::new(1024, 128, 4, 1);
    assert!(!pool.game_mode());

    pool.set_game_mode(true);
    assert!(pool.game_mode());
    assert_eq!(pool.effective_limit(), 128 * MB);
    assert_eq!(pool.effective_max_parallel(), 1);

    pool.set_game_mode(false);
    assert!(!pool.game_mode());
    assert_eq!(pool.effective_limit(), 1024 * MB);
    assert_eq!(pool.effective_max_parallel(), 4);
}

#[tokio::test]
#[timeout(10000)]
async fn test_game_mode_active_slots_unaffected() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
    let _g1 = pool.acquire_slot().await;
    let _g2 = pool.acquire_slot().await;
    assert_eq!(pool.active_slots(), 2);

    pool.set_game_mode(true);
    assert_eq!(pool.active_slots(), 2);
    assert_eq!(pool.effective_max_parallel(), 1);
}

// -----------------------------------------------------------------------
// update_limits
// -----------------------------------------------------------------------

#[tokio::test]
#[timeout(10000)]
async fn test_update_limits() {
    let pool = BufferPool::new(1024, 128, 4, 1);
    assert_eq!(pool.effective_limit(), 1024 * MB);
    assert_eq!(pool.effective_max_parallel(), 4);

    pool.update_limits(512, 64, 2, 1);
    assert_eq!(pool.effective_limit(), 512 * MB);
    assert_eq!(pool.effective_max_parallel(), 2);

    pool.set_game_mode(true);
    assert_eq!(pool.effective_limit(), 64 * MB);
    assert_eq!(pool.effective_max_parallel(), 1);

    pool.set_game_mode(false);
    assert_eq!(pool.effective_limit(), 512 * MB);
    assert_eq!(pool.effective_max_parallel(), 2);
}

#[tokio::test]
#[timeout(10000)]
async fn test_update_limits_affects_half_size() {
    let pool = BufferPool::new(1024, 128, 4, 1);
    let original = pool.half_size();

    pool.update_limits(512, 128, 4, 1);
    let new_half = pool.half_size();
    assert!(new_half < original);
}

// -----------------------------------------------------------------------
// queued_count
// -----------------------------------------------------------------------

#[tokio::test]
#[timeout(10000)]
async fn test_queued_count_basic() {
    let pool = BufferPool::new(1024, 128, 4, 1);
    assert_eq!(pool.queued_count(), 0);

    let _g1 = pool.acquire_slot().await;
    let _g2 = pool.acquire_slot().await;
    assert_eq!(pool.queued_count(), 0);
}

// -----------------------------------------------------------------------
// SlotGuard — semaphore permit release on drop
// -----------------------------------------------------------------------

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

// -----------------------------------------------------------------------
// DownloadBuffer — HDD (Double) mode
// -----------------------------------------------------------------------

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

// -----------------------------------------------------------------------
// flush_all
// -----------------------------------------------------------------------

#[tokio::test]
#[timeout(10000)]
async fn test_flush_all_hdd_empty() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
    let (_dir, file) = temp_file();
    let slot = pool.acquire_slot().await;
    let buf = DownloadBuffer::new(pool.clone(), slot, file);

    buf.flush_all().await.unwrap();
}

// -----------------------------------------------------------------------
// clear()
// -----------------------------------------------------------------------

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

// -----------------------------------------------------------------------
// DownloadBuffer — SSD (LocalPingPong) mode
// -----------------------------------------------------------------------

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

// -----------------------------------------------------------------------
// drain_background
// -----------------------------------------------------------------------

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
async fn test_drain_background_noop_when_no_background_task() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
    let (_dir, file) = temp_file();
    let slot = pool.acquire_slot().await;
    let buf = DownloadBuffer::new(pool.clone(), slot, file);

    buf.drain_background().await;
}

// -----------------------------------------------------------------------
// DownloadBuffer Drop — releases slot
// -----------------------------------------------------------------------

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

// -----------------------------------------------------------------------
// Zero-size chunk
// -----------------------------------------------------------------------

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

// -----------------------------------------------------------------------
// Concurrent buffer_chunk calls
// -----------------------------------------------------------------------

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

// -----------------------------------------------------------------------
// Game mode transition while slots are held
// -----------------------------------------------------------------------

#[tokio::test]
#[timeout(10000)]
async fn test_game_mode_transition_with_held_slots() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));

    let _g1 = pool.acquire_slot().await;
    let _g2 = pool.acquire_slot().await;
    assert_eq!(pool.active_slots(), 2);
    assert_eq!(pool.effective_max_parallel(), 4);

    pool.set_game_mode(true);
    assert_eq!(pool.effective_max_parallel(), 1);
    assert_eq!(pool.active_slots(), 2);

    let game_half = pool.half_size();
    assert_eq!(game_half, (128 * MB / 2).max(64 * KB));

    pool.set_game_mode(false);
    assert_eq!(pool.effective_max_parallel(), 4);
    assert_eq!(pool.active_slots(), 2);
}

#[tokio::test]
#[timeout(10000)]
async fn test_game_mode_affects_new_buffers_only() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));

    let (_dir1, file1) = temp_file();
    let slot1 = pool.acquire_slot().await;
    let _buf1 = DownloadBuffer::new(pool.clone(), slot1, file1);

    pool.set_game_mode(true);

    let (_dir2, file2) = temp_file();
    let slot2 = pool.acquire_slot().await;
    let _buf2 = DownloadBuffer::new(pool.clone(), slot2, file2);

    assert_eq!(pool.half_size(), (128 * MB / 2).max(64 * KB));
    assert_eq!(pool.effective_max_parallel(), 1);
}

// -----------------------------------------------------------------------
// Pool memory management integration
// -----------------------------------------------------------------------

#[tokio::test]
#[timeout(10000)]
async fn test_pool_usage_tracked_across_multiple_buffers() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
    let (_dir1, file1) = temp_file();
    let (_dir2, file2) = temp_file();

    let slot1 = pool.acquire_slot().await;
    let slot2 = pool.acquire_slot().await;

    {
        let buf1 = DownloadBuffer::new(pool.clone(), slot1, file1);
        let buf2 = DownloadBuffer::new(pool.clone(), slot2, file2);

        buf1.buffer_chunk(0, Bytes::from("hello")).await.unwrap();
        buf2.buffer_chunk(0, Bytes::from("world")).await.unwrap();
        assert_eq!(pool.current_usage(), 10);
    }
    assert_eq!(pool.current_usage(), 0);
    assert_eq!(pool.active_slots(), 0);
}

// -----------------------------------------------------------------------
// Integrity: flush with overlapping offsets
// -----------------------------------------------------------------------

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

// -----------------------------------------------------------------------
// Degradation / error-recovery tests
// -----------------------------------------------------------------------

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

#[tokio::test]
#[timeout(10000)]
async fn test_max_slots_matches_effective_max_parallel() {
    let pool = BufferPool::new(1024, 128, 4, 1);
    assert_eq!(pool.max_slots(), pool.effective_max_parallel());

    pool.set_game_mode(true);
    assert_eq!(pool.max_slots(), pool.effective_max_parallel());

    pool.update_limits(1024, 128, 8, 2);
    assert_eq!(pool.max_slots(), pool.effective_max_parallel());

    pool.set_game_mode(false);
    assert_eq!(pool.max_slots(), pool.effective_max_parallel());
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

#[test]
fn flip_token_recovers_from_panic() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let token = AtomicBool::new(true);
    let notify = tokio::sync::Notify::new();

    let result = catch_unwind(AssertUnwindSafe(|| {
        let _guard = FlipTokenGuard {
            token: &token,
            notify: &notify,
        };
        panic!("simulated flip section panic");
    }));

    assert!(result.is_err(), "expected panic to be caught");
    assert!(
        !token.load(Ordering::Acquire),
        "flip_token should be false after guard drop on unwind"
    );
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

#[test]
fn test_write_coalesced_entries_contiguous() {
    use crate::buffer_pool::worker::write_coalesced_entries;

    let (_dir, file) = temp_file();
    let entries = vec![
        (0u64, Bytes::from_static(b"Hello ")),
        (6u64, Bytes::from_static(b"World ")),
        (12u64, Bytes::from_static(b"from LimeDL!")),
    ];

    write_coalesced_entries(&file, &entries).expect("coalesced write succeeds");
    drop(file);

    let content = fs::read(_dir.path().join("test.bin")).expect("read test file");
    assert_eq!(content, b"Hello World from LimeDL!");
}

#[test]
fn test_write_coalesced_entries_disjoint() {
    use crate::buffer_pool::worker::write_coalesced_entries;

    let (_dir, file) = temp_file();
    let entries = vec![
        (0u64, Bytes::from_static(b"AAAA")),
        (10u64, Bytes::from_static(b"BBBB")),
        (14u64, Bytes::from_static(b"CCCC")),
    ];

    write_coalesced_entries(&file, &entries).expect("coalesced write succeeds");
    drop(file);

    let content = fs::read(_dir.path().join("test.bin")).expect("read test file");
    assert_eq!(&content[0..4], b"AAAA");
    assert_eq!(&content[10..18], b"BBBBCCCC");
}
