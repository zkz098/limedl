//! Progress recording and cancellation outcomes.

use super::*;

#[tokio::test]
#[timeout(30_000)]
async fn record_progress_normal_update() {
    let managed = make_managed_with_chunk("dl1", 0, 0, 4_194_304, 0, Some(8_388_608));

    record_progress_on_managed(&managed, Some(0), 1000);

    let core = managed.core.lock();
    assert_eq!(core.snapshot.downloaded_bytes, 1000);
    assert_eq!(core.manifest.downloaded_bytes, 1000);
    assert_eq!(core.manifest.chunks[0].downloaded, 1000);
    assert!(!core.manifest.chunks[0].completed);
    assert!(core.manifest.chunks[0].dirty);
    assert!(core.snapshot.updated_at_ms > 0);
}

#[tokio::test]
#[timeout(30_000)]
async fn record_progress_chunk_index_beyond_range() {
    let managed = make_managed_with_chunk("dl2", 500, 0, 4_194_304, 100, Some(8_388_608));

    // Use a chunk index that doesn't exist
    record_progress_on_managed(&managed, Some(999), 2000);

    let core = managed.core.lock();
    // Snapshot and manifest still update
    assert_eq!(core.snapshot.downloaded_bytes, 2500);
    assert_eq!(core.manifest.downloaded_bytes, 2500);
    // Chunk remains unchanged
    assert_eq!(core.manifest.chunks[0].downloaded, 100);
    assert!(!core.manifest.chunks[0].completed);
}

#[tokio::test]
#[timeout(30_000)]
async fn record_progress_chunk_exceeds_bounds_marks_completed() {
    // Chunk covers 0..1_000_000, currently at 999_500
    let managed = make_managed_with_chunk("dl3", 999_500, 0, 1_000_000, 999_500, Some(10_000_000));

    // Add 1000 bytes — pushes chunk downloaded (1_000_500) past its size (1_000_000)
    record_progress_on_managed(&managed, Some(0), 1000);

    let core = managed.core.lock();
    assert_eq!(core.manifest.chunks[0].downloaded, 1_000_500);
    assert!(
        core.manifest.chunks[0].completed,
        "chunk should be marked completed when downloaded > size"
    );
    assert!(
        core.manifest.chunks[0].claimed_by.is_none(),
        "claimed_by should be cleared"
    );
}

#[tokio::test]
#[timeout(30_000)]
async fn record_progress_overflow_protection() {
    let managed = make_managed_with_chunk("dl4", u64::MAX, 0, 4_194_304, 0, Some(8_388_608));

    record_progress_on_managed(&managed, Some(0), 5000);

    let core = managed.core.lock();
    // saturating_add should keep value at u64::MAX
    assert_eq!(core.snapshot.downloaded_bytes, u64::MAX);
    assert_eq!(core.manifest.downloaded_bytes, u64::MAX);
}

#[tokio::test]
#[timeout(30_000)]
async fn record_progress_none_chunk_index_still_updates_snapshot() {
    let managed = make_managed_with_chunk("dl5", 100, 0, 4_194_304, 50, Some(8_388_608));

    record_progress_on_managed(&managed, None, 777);

    let core = managed.core.lock();
    assert_eq!(core.snapshot.downloaded_bytes, 877);
    assert_eq!(core.manifest.downloaded_bytes, 877);
    // Chunk should be untouched
    assert_eq!(core.manifest.chunks[0].downloaded, 50);
}

#[tokio::test]
#[timeout(30_000)]
async fn cancellation_outcome_when_canceled() {
    let managed = make_managed("canceled", DownloadState::Canceled, "https://example.com/f");
    let outcome = cancellation_outcome(&managed);
    assert!(matches!(outcome, RunOutcome::Canceled));
}

#[tokio::test]
#[timeout(30_000)]
async fn cancellation_outcome_when_downloading() {
    let managed = make_managed("dl", DownloadState::Downloading, "https://example.com/f");
    let outcome = cancellation_outcome(&managed);
    assert!(matches!(outcome, RunOutcome::Paused));
}

#[tokio::test]
#[timeout(30_000)]
async fn cancellation_outcome_when_paused() {
    let managed = make_managed("paused", DownloadState::Paused, "https://example.com/f");
    let outcome = cancellation_outcome(&managed);
    assert!(matches!(outcome, RunOutcome::Paused));
}

#[tokio::test]
#[timeout(30_000)]
async fn cancellation_outcome_when_completed() {
    let managed = make_managed("done", DownloadState::Completed, "https://example.com/f");
    let outcome = cancellation_outcome(&managed);
    assert!(matches!(outcome, RunOutcome::Paused));
}

#[tokio::test]
#[timeout(30_000)]
async fn cancellation_chunk_outcome_when_canceled() {
    let managed = make_managed("canceled", DownloadState::Canceled, "https://example.com/f");
    let outcome = cancellation_chunk_outcome(&managed);
    assert!(matches!(outcome, ChunkWorkerOutcome::Canceled));
}

#[tokio::test]
#[timeout(30_000)]
async fn cancellation_chunk_outcome_when_downloading() {
    let managed = make_managed("dl", DownloadState::Downloading, "https://example.com/f");
    let outcome = cancellation_chunk_outcome(&managed);
    assert!(matches!(outcome, ChunkWorkerOutcome::Paused));
}
