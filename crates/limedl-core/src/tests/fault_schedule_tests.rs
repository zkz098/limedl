//! Randomized fault-matrix E2E tests.
//!
//! The deterministic suites pin one server fault at a time. This suite asks the
//! complementary question: across randomized file sizes, worker counts, retry
//! budgets and server faults, does the engine ever report `Completed` for bytes
//! that differ from the source?
//!
//! The oracle is deliberately the same one the corruption tests use: re-read the
//! destination file from disk, hash it independently, and compare it with the
//! deterministic source. A corrupt `Completed` is a bug regardless of which
//! fault caused it. Faults that are documented to fail (`416`, permanent 4xx)
//! must stay `Failed`; faults the engine is expected to recover from
//! (one-shot 500, a chunked body without `Content-Length`) must complete.
//!
//! Adaptive chunking is 4 MiB below 256 MiB and parallelism needs two chunks, so
//! cases start at 8 MiB. That is the range-worker path where a misbehaving
//! server can hand back wrong bytes while every per-chunk byte count stays
//! correct.

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use proptest::prelude::*;
use tempfile::TempDir;
use tokio::runtime::Runtime;
use tokio::time::sleep;

use crate::DownloadManager;
use crate::checksum::hash_slices;
use crate::event_bus::EventBus;
use crate::rate_limiter::RateLimiter;
use crate::test_harness::TestServer;
use crate::types::{ChecksumMode, DownloadState, StartDownloadRequest};

const MIB: u64 = 1024 * 1024;

#[derive(Debug, Clone)]
enum Fault {
    Healthy,
    /// One-shot 500 on the first range request at or beyond `percent` of the
    /// file; later requests succeed.
    FailOnce {
        percent: u8,
    },
    Range416,
    /// Serve range content shifted relative to the advertised `Content-Range`.
    Shifted {
        shift: u64,
    },
    /// Flip the first byte of every range.
    Bitflip,
    /// Permanent client error: classified as invalid, so it fails without
    /// burning the case budget in retry backoff.
    HttpClientError {
        code: u16,
    },
    /// Full body without `Content-Length` (chunked transfer encoding).
    NoLength,
}

impl Fault {
    fn expects_completion(&self) -> bool {
        matches!(self, Self::Healthy | Self::FailOnce { .. } | Self::NoLength)
    }

    fn must_fail(&self) -> bool {
        matches!(self, Self::Range416 | Self::HttpClientError { .. })
    }
}

#[derive(Debug, Clone)]
struct FaultCase {
    size_mib: u64,
    threads: usize,
    max_retries: u32,
    fault: Fault,
}

fn fault_strategy() -> impl Strategy<Value = Fault> {
    prop_oneof![
        Just(Fault::Healthy),
        (1u8..=90).prop_map(|percent| Fault::FailOnce { percent }),
        Just(Fault::Range416),
        (1u64..=64).prop_map(|shift| Fault::Shifted { shift }),
        Just(Fault::Bitflip),
        // Permanent 4xx: `classify_download_response` only retries 408/429 and
        // 5xx, so these fail promptly instead of spending the case budget in
        // exponential backoff.
        prop_oneof![
            Just(400u16),
            Just(401),
            Just(403),
            Just(404),
            Just(410),
            Just(418),
            Just(451),
        ]
        .prop_map(|code| Fault::HttpClientError { code }),
        Just(Fault::NoLength),
    ]
}

fn case_strategy() -> impl Strategy<Value = FaultCase> {
    (8u64..=16, 1usize..=8, 1u32..=3, fault_strategy()).prop_map(
        |(size_mib, threads, max_retries, fault)| FaultCase {
            size_mib,
            threads,
            max_retries,
            fault,
        },
    )
}

/// One runtime for the whole proptest run; building one per case would dominate
/// the suite's runtime.
fn runtime() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| Runtime::new().expect("fault-schedule runtime"))
}

fn url_for(server: &TestServer, fault: &Fault, size: u64) -> String {
    match fault {
        Fault::Healthy => server.file_url_range(),
        Fault::FailOnce { percent } => {
            let after = (size * u64::from(*percent) / 100).max(1);
            server.file_url_range_fail_once(after)
        }
        Fault::Range416 => server.file_url_range_416(),
        Fault::Shifted { shift } => server.file_url_range_shifted(*shift),
        Fault::Bitflip => server.file_url_range_bitflip(),
        Fault::HttpClientError { code } => server.file_url_status(*code),
        Fault::NoLength => server.file_url_no_length(),
    }
}

async fn wait_for_terminal(manager: &DownloadManager, id: &str) -> crate::types::DownloadSnapshot {
    let start = Instant::now();
    loop {
        let status = manager.status(id).await.expect("status");
        if matches!(
            status.state,
            DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
        ) {
            return status;
        }
        assert!(
            start.elapsed() < Duration::from_secs(60),
            "download {id} did not reach a terminal state"
        );
        sleep(Duration::from_millis(25)).await;
    }
}

async fn run_case(case: FaultCase) {
    let size = case.size_mib * MIB;
    let server = TestServer::new(size).await;
    let url = url_for(&server, &case.fault, size);

    let temp = TempDir::new().expect("temp dir");
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let dest_dir = temp.path().join("out");
    std::fs::create_dir_all(&dest_dir).expect("output dir");

    let manager = Arc::new(
        DownloadManager::new_with_components(
            temp.path().join("state"),
            Arc::new(RateLimiter::default()),
            Arc::new(EventBus::new(1024)),
        )
        .expect("manager"),
    );

    let request = StartDownloadRequest {
        kind: None,
        url,
        destination_dir: dest_dir.to_string_lossy().to_string(),
        file_name: Some("test.bin".into()),
        thread_mode: None,
        thread_count: Some(case.threads),
        max_retries: Some(case.max_retries),
        checksum: Some(ChecksumMode::Sha256),
        expected_checksum: Some(server.sha256_hash.clone()),
        selected_file_indices: None,
        start_paused: false,
        headers: None,
        mirror_urls: None,
        user_agent: None,
        priority: None,
    };
    let id = manager.start(request).await.expect("start failed");
    let snap = wait_for_terminal(&manager, &id.to_string()).await;

    assert!(
        snap.downloaded_bytes <= size,
        "received {} bytes for a {} byte file (case {case:?})",
        snap.downloaded_bytes,
        size
    );

    match snap.state {
        DownloadState::Completed => {
            let path = PathBuf::from(&snap.destination_path);
            let bytes = std::fs::read(&path).unwrap_or_else(|e| {
                panic!(
                    "completed download has no readable file at {}: {e}",
                    path.display()
                )
            });
            assert_eq!(
                bytes.len() as u64,
                size,
                "completed download length {} != source length {size} (case {case:?})",
                bytes.len()
            );
            assert_eq!(
                hash_slices(ChecksumMode::Sha256, &[&bytes]),
                server.sha256_hash,
                "completed download differs from source (case {case:?})"
            );
        }
        DownloadState::Failed => {
            assert!(
                !case.fault.expects_completion(),
                "expected completion but got Failed: {:?} (case {case:?})",
                snap.error
            );
        }
        other => panic!("unexpected terminal state {other:?} (case {case:?})"),
    }

    if case.fault.must_fail() {
        assert!(
            matches!(snap.state, DownloadState::Failed),
            "fault must fail, got {:?} (case {case:?})",
            snap.state
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        // A case starts an HTTP server and transfers up to 16 MiB, so this is
        // deliberately small: it runs in the normal gate, not as a soak. Bump
        // with PROPTEST_CASES=200 for a targeted hunt.
        cases: 16,
        max_shrink_iters: 64,
        ..ProptestConfig::default()
    })]

    #[test]
    fn fault_matrix_never_completes_with_wrong_bytes(case in case_strategy()) {
        runtime().block_on(run_case(case));
    }
}
