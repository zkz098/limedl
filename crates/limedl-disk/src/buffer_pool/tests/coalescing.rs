//! Background drain and flip-token recovery.

use super::*;

#[tokio::test]
async fn test_drain_background_noop_when_no_background_task() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
    let (_dir, file) = temp_file();
    let slot = pool.acquire_slot().await;
    let buf = DownloadBuffer::new(pool.clone(), slot, file);

    buf.drain_background().await;
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
async fn flip_token_pre_enabled_notify_prevents_lost_wakeup() {
    let token = AtomicBool::new(true);
    let notify = tokio::sync::Notify::new();

    // Simulate a waiter enabling its `Notified` future BEFORE checking `token.swap(true)`.
    let notified = notify.notified();
    tokio::pin!(notified);
    notified.as_mut().enable();

    assert!(
        token.swap(true, Ordering::SeqCst),
        "token should still be held by the simulated flipper"
    );

    // Flipper finishes and drops `FlipTokenGuard` BEFORE the waiter polls `notified.await`.
    {
        let _guard = FlipTokenGuard {
            token: &token,
            notify: &notify,
        };
    }

    // Without `enable()` before `swap(true)`, this `.await` would hang forever.
    tokio::time::timeout(std::time::Duration::from_millis(100), notified)
        .await
        .expect("pre-enabled Notified must not lose the wakeup from FlipTokenGuard::drop");
    assert!(!token.load(Ordering::SeqCst));
}

#[tokio::test]
async fn overlapping_writes_preserve_exact_buffer_and_pool_usage() {
    let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
    let (_dir, file) = temp_file();
    let slot = pool.acquire_slot().await;
    let buf = DownloadBuffer::new(pool.clone(), slot, file);

    buf.buffer_chunk(0, Bytes::from("12345")).await.unwrap();
    assert_eq!(buf.len(), 5);
    assert_eq!(pool.current_usage(), 5);

    // Overwrite offset 0 with a larger chunk: usage must be 8, not 5 + 8 = 13.
    buf.buffer_chunk(0, Bytes::from("12345678")).await.unwrap();
    assert_eq!(buf.len(), 8);
    assert_eq!(pool.current_usage(), 8);

    // Overwrite offset 0 with a smaller chunk: usage must be 3.
    buf.buffer_chunk(0, Bytes::from("abc")).await.unwrap();
    assert_eq!(buf.len(), 3);
    assert_eq!(pool.current_usage(), 3);

    buf.flush_all().await.unwrap();
    assert_eq!(buf.len(), 0);
    assert_eq!(pool.current_usage(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_multi_flip_contention_no_lost_wakeup_or_leak() {
    // 64 KiB half_size; 32 concurrent writers × 8 KiB = 256 KiB (4 flips under contention).
    let pool = Arc::new(BufferPool::new(0, 0, 1, 1));
    let (_dir, file) = temp_file();
    let slot = pool.acquire_slot().await;
    let worker = test_io_worker();
    let buf = Arc::new(DownloadBuffer::new_with_worker(
        pool.clone(),
        slot,
        file,
        temp_file_path(&_dir),
        worker,
        None,
    ));

    let chunk_size = 8 * KB;
    let num_tasks = 32u64;
    let mut handles = Vec::new();
    for i in 0..num_tasks {
        let b = Arc::clone(&buf);
        handles.push(tokio::spawn(async move {
            let payload = vec![(i + 1) as u8; chunk_size as usize];
            b.buffer_chunk(i * chunk_size, Bytes::from(payload))
                .await
                .unwrap();
        }));
    }

    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        for h in handles {
            h.await.unwrap();
        }
    })
    .await
    .expect("concurrent flips must not hang on lost Notify wakeup");

    buf.flush_all().await.unwrap();
    assert_eq!(buf.len(), 0);
    assert_eq!(pool.current_usage(), 0);

    let on_disk = fs::read(_dir.path().join("test.bin")).unwrap();
    assert_eq!(on_disk.len() as u64, num_tasks * chunk_size);
    for i in 0..num_tasks {
        let off = (i * chunk_size) as usize;
        assert_eq!(on_disk[off], (i + 1) as u8);
    }
}
