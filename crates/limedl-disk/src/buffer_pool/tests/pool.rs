//! BufferPool limits, slots and game mode.

use super::*;

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

#[tokio::test]
#[timeout(10000)]
async fn test_queued_count_basic() {
    let pool = BufferPool::new(1024, 128, 4, 1);
    assert_eq!(pool.queued_count(), 0);

    let _g1 = pool.acquire_slot().await;
    let _g2 = pool.acquire_slot().await;
    assert_eq!(pool.queued_count(), 0);
}

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
