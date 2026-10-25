//! aria2/irontide state and mode mappings.

use super::*;

#[test]
fn test_map_state_downloading() {
    assert_eq!(
        map_state(&irontide::session::TorrentState::Downloading),
        DownloadState::Downloading
    );
}

#[test]
fn test_map_state_seeding() {
    assert_eq!(
        map_state(&irontide::session::TorrentState::Seeding),
        DownloadState::Completed
    );
}

#[test]
fn test_map_state_complete() {
    assert_eq!(
        map_state(&irontide::session::TorrentState::Complete),
        DownloadState::Completed
    );
}

#[test]
fn test_map_state_paused() {
    assert_eq!(
        map_state(&irontide::session::TorrentState::Paused),
        DownloadState::Paused
    );
}

#[test]
fn test_map_state_checking() {
    assert_eq!(
        map_state(&irontide::session::TorrentState::Checking),
        DownloadState::Verifying
    );
}

#[test]
fn test_map_state_fetching_metadata() {
    assert_eq!(
        map_state(&irontide::session::TorrentState::FetchingMetadata),
        DownloadState::Queued
    );
}

#[test]
fn test_map_state_queued() {
    assert_eq!(
        map_state(&irontide::session::TorrentState::Queued),
        DownloadState::Queued
    );
}

#[test]
fn test_map_state_stopped() {
    assert_eq!(
        map_state(&irontide::session::TorrentState::Stopped),
        DownloadState::Canceled
    );
}

#[test]
fn test_map_state_sharing() {
    assert_eq!(
        map_state(&irontide::session::TorrentState::Sharing),
        DownloadState::Downloading
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

#[test]
fn test_map_state_all_variants_exhaustive() {
    // Verify every irontide TorrentState maps to a DownloadState
    // without panicking or returning unexpected values.
    use irontide::session::TorrentState;
    let cases: Vec<(TorrentState, DownloadState)> = vec![
        (TorrentState::Downloading, DownloadState::Downloading),
        (TorrentState::Seeding, DownloadState::Completed),
        (TorrentState::Complete, DownloadState::Completed),
        (TorrentState::Paused, DownloadState::Paused),
        (TorrentState::Checking, DownloadState::Verifying),
        (TorrentState::FetchingMetadata, DownloadState::Queued),
        (TorrentState::Queued, DownloadState::Queued),
        (TorrentState::Stopped, DownloadState::Canceled),
        (TorrentState::Sharing, DownloadState::Downloading),
    ];
    for (input, expected) in &cases {
        assert_eq!(map_state(input), *expected, "TorrentState::{input:?}");
    }
}

#[test]
fn test_encryption_mode_mapping_enabled() {
    use crate::types::BtEncryptionMode;
    let builder = irontide::ClientBuilder::new().encryption_mode(match BtEncryptionMode::Enabled {
        BtEncryptionMode::Enabled => irontide::prelude::EncryptionMode::Enabled,
        BtEncryptionMode::Disabled => irontide::prelude::EncryptionMode::Disabled,
        BtEncryptionMode::Forced => irontide::prelude::EncryptionMode::Forced,
    });
    let settings = builder.into_settings();
    assert_eq!(
        settings.encryption_mode,
        irontide::prelude::EncryptionMode::Enabled
    );
}

#[test]
fn test_encryption_mode_mapping_disabled() {
    use crate::types::BtEncryptionMode;
    let builder =
        irontide::ClientBuilder::new().encryption_mode(match BtEncryptionMode::Disabled {
            BtEncryptionMode::Enabled => irontide::prelude::EncryptionMode::Enabled,
            BtEncryptionMode::Disabled => irontide::prelude::EncryptionMode::Disabled,
            BtEncryptionMode::Forced => irontide::prelude::EncryptionMode::Forced,
        });
    let settings = builder.into_settings();
    assert_eq!(
        settings.encryption_mode,
        irontide::prelude::EncryptionMode::Disabled
    );
}

#[test]
fn test_encryption_mode_mapping_forced() {
    use crate::types::BtEncryptionMode;
    let builder = irontide::ClientBuilder::new().encryption_mode(match BtEncryptionMode::Forced {
        BtEncryptionMode::Enabled => irontide::prelude::EncryptionMode::Enabled,
        BtEncryptionMode::Disabled => irontide::prelude::EncryptionMode::Disabled,
        BtEncryptionMode::Forced => irontide::prelude::EncryptionMode::Forced,
    });
    let settings = builder.into_settings();
    assert_eq!(
        settings.encryption_mode,
        irontide::prelude::EncryptionMode::Forced
    );
}

#[test]
fn test_preallocate_mode_mapping_none() {
    use crate::types::BtPreallocateMode;
    let builder = irontide::ClientBuilder::new().preallocate_mode(match BtPreallocateMode::None {
        BtPreallocateMode::None => irontide::prelude::PreallocateMode::None,
        BtPreallocateMode::Full => irontide::prelude::PreallocateMode::Full,
    });
    let settings = builder.into_settings();
    assert_eq!(
        settings.preallocate_mode,
        irontide::prelude::PreallocateMode::None
    );
}

#[test]
fn test_preallocate_mode_mapping_full() {
    use crate::types::BtPreallocateMode;
    let builder = irontide::ClientBuilder::new().preallocate_mode(match BtPreallocateMode::Full {
        BtPreallocateMode::None => irontide::prelude::PreallocateMode::None,
        BtPreallocateMode::Full => irontide::prelude::PreallocateMode::Full,
    });
    let settings = builder.into_settings();
    assert_eq!(
        settings.preallocate_mode,
        irontide::prelude::PreallocateMode::Full
    );
}
