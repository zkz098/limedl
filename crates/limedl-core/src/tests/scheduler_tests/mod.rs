//! Integration tests for the scheduler (rebalance logic) via DownloadManager.
//!
//! Each test creates a real HTTP server (via [`TestServer`]), a real
//! [`DownloadManager`], and exercises scheduler behaviour through
//! the public `start` / `pause` / `resume` / `cancel` / `status` API.

use std::sync::Arc;
use std::time::Duration;

use ntest::timeout;
use tempfile::tempdir;

use crate::{
    event_bus::EventBus,
    manager::DownloadManager,
    rate_limiter::RateLimiter,
    test_harness::TestServer,
    types::{
        AdaptiveProfile, AppSettings, AutomaticSchedulerSettings, ChecksumMode, DownloadSnapshot,
        DownloadState, ProxyMode, ProxySettings, SchedulerMode, SchedulerSettings,
        StartDownloadRequest, ThreadMode, TraditionalSchedulerSettings,
    },
};

type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

// ---------------------------------------------------------------------------
// Manager helpers
// ---------------------------------------------------------------------------

async fn create_manager() -> (tempfile::TempDir, Arc<DownloadManager>) {
    let tmp = tempdir().expect("tempdir");
    let state_dir = tmp.path().join("state");
    std::fs::create_dir_all(state_dir.join("logs")).ok();
    let manager = Arc::new(
        DownloadManager::new_with_components(
            state_dir,
            Arc::new(RateLimiter::default()),
            Arc::new(EventBus::new(1024)),
        )
        .expect("DownloadManager::new"),
    );
    (tmp, manager)
}

async fn apply_settings(manager: &DownloadManager, scheduler: SchedulerSettings) {
    manager
        .apply_settings(AppSettings {
            scheduler,
            ..AppSettings::default()
        })
        .await
        .expect("update_settings");
}

fn req_fixed(url: &str, dir: &str, name: &str) -> StartDownloadRequest {
    StartDownloadRequest {
        kind: None,
        url: url.to_string(),
        destination_dir: dir.to_string(),
        file_name: Some(name.to_string()),
        user_agent: None,
        thread_mode: Some(ThreadMode::Fixed),
        thread_count: Some(4),
        max_retries: Some(1),
        checksum: Some(ChecksumMode::None),
        expected_checksum: None,
        selected_file_indices: None,
        start_paused: false,
        headers: None,
        mirror_urls: None,
        priority: None,
    }
}

/// Convenience helper for range-supported multi-threaded downloads.
fn req_fixed_range(url: &str, out: &str, name: &str) -> StartDownloadRequest {
    StartDownloadRequest {
        kind: None,
        url: url.to_string(),
        destination_dir: out.to_string(),
        file_name: Some(name.to_string()),
        user_agent: None,
        thread_mode: Some(ThreadMode::Fixed),
        thread_count: Some(4),
        max_retries: Some(1),
        checksum: Some(ChecksumMode::None),
        expected_checksum: None,
        selected_file_indices: None,
        start_paused: false,
        headers: None,
        mirror_urls: None,
        priority: None,
    }
}

/// Convenience helper for adaptive downloads (Automatic scheduler).
fn req_adaptive(url: &str, out: &str, name: &str) -> StartDownloadRequest {
    StartDownloadRequest {
        kind: None,
        url: url.to_string(),
        destination_dir: out.to_string(),
        file_name: Some(name.to_string()),
        user_agent: None,
        thread_mode: Some(ThreadMode::Adaptive),
        thread_count: None,
        max_retries: Some(1),
        checksum: Some(ChecksumMode::None),
        expected_checksum: None,
        selected_file_indices: None,
        start_paused: false,
        headers: None,
        mirror_urls: None,
        priority: None,
    }
}

// ---------------------------------------------------------------------------
// Polling helpers
// ---------------------------------------------------------------------------

async fn wait_for_terminal(manager: &DownloadManager, id: &str) -> DownloadSnapshot {
    loop {
        let s = manager.status(id).await.unwrap();
        if matches!(
            s.state,
            DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
        ) {
            return s;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
// ===========================================================================
// Test 2: Automatic mode prioritises larger file
// ===========================================================================

// ===========================================================================
// Test 3: Pause / resume preserves progress
// ===========================================================================

// ===========================================================================
// Test 4: Cancel stops a download
// ===========================================================================

// ===========================================================================
// Test 5: Global speed limit is respected
// ===========================================================================

// ===========================================================================
// Test 7: Multi-download fairness under limited threads
// ===========================================================================

// ===========================================================================
// Test 8: Mixed thread-mode downloads coexist
// ===========================================================================

// ===========================================================================
// Test 9: Rate limiter shared across multiple downloads
// ===========================================================================

// ===========================================================================
// Test 10: Pausing one download does not affect the other
// ===========================================================================

// ===========================================================================
// Test 11: Cancelling one download unblocks a queued download
// ===========================================================================

// ===========================================================================
// Test 12: A configured proxy does not disable adaptive tuning
// ===========================================================================

mod lifecycle;
mod limits;
mod scheduling_e2e;
