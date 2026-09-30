//! Write coalescing and background drain.

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
