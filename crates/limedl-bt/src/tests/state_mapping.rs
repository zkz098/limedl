//! aria2/librqbit state and mode mappings.

use super::*;
use librqbit::{TorrentStats, TorrentStatsState};

fn make_stats(state: TorrentStatsState, finished: bool) -> TorrentStats {
    TorrentStats {
        state,
        file_progress: Default::default(),
        error: None,
        progress_bytes: 0,
        uploaded_bytes: 0,
        finished,
        total_bytes: 0,
        live: None,
    }
}

#[test]
fn test_map_state_downloading() {
    assert_eq!(
        map_state(&make_stats(TorrentStatsState::Live, false)),
        DownloadState::Downloading
    );
}

#[test]
fn test_map_state_finished() {
    assert_eq!(
        map_state(&make_stats(TorrentStatsState::Live, true)),
        DownloadState::Completed
    );
}

#[test]
fn test_map_state_paused() {
    assert_eq!(
        map_state(&make_stats(TorrentStatsState::Paused, false)),
        DownloadState::Paused
    );
}

#[test]
fn test_map_state_initializing_paused() {
    assert_eq!(
        map_state(&make_stats(
            TorrentStatsState::Initializing { paused: true },
            false
        )),
        DownloadState::Paused
    );
}

#[test]
fn test_map_state_initializing_verifying() {
    assert_eq!(
        map_state(&make_stats(
            TorrentStatsState::Initializing { paused: false },
            false
        )),
        DownloadState::Verifying
    );
}

#[test]
fn test_map_state_error() {
    assert_eq!(
        map_state(&make_stats(TorrentStatsState::Error, false)),
        DownloadState::Failed
    );
}

#[test]
fn test_state_helpers_is_terminal_completed() {
    assert!(DownloadState::Completed.is_terminal());
}

#[test]
fn test_state_helpers_is_terminal_failed() {
    assert!(DownloadState::Failed.is_terminal());
}

#[test]
fn test_state_helpers_is_terminal_canceled() {
    assert!(DownloadState::Canceled.is_terminal());
}

#[test]
fn test_state_helpers_is_terminal_downloading() {
    assert!(!DownloadState::Downloading.is_terminal());
}

#[test]
fn test_state_helpers_is_terminal_paused() {
    assert!(!DownloadState::Paused.is_terminal());
}

#[test]
fn test_state_helpers_is_terminal_queued() {
    assert!(!DownloadState::Queued.is_terminal());
}

#[test]
fn test_state_helpers_is_terminal_verifying() {
    assert!(!DownloadState::Verifying.is_terminal());
}
