//! Background drain and flip-token recovery.

use super::*;

#[tokio::test]
#[timeout(10000)]
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
