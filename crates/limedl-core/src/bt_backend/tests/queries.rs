//! `queries.rs` surface: torrent preview, peers/trackers/pieces/files, speed
//! limits, runtime status and the pending-summary fallback.
//!
//! These all go through `tokio::task::block_in_place`, so they need the
//! multi-thread runtime (`#[tokio::test]` defaults to current-thread, where
//! `block_in_place` panics).

use super::*;

use crate::error::DownloadError;
use crate::event_bus::DownloadEvent;

/// A valid, metadata-less magnet request.
fn magnet_request(dn: &str) -> StartDownloadRequest {
    StartDownloadRequest {
        url: format!("magnet:?xt=urn:btih:aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d&dn={dn}"),
        ..Default::default()
    }
}

/// Add the fixture `.torrent` (metadata present, no network) to `backend`.
async fn start_fixture(backend: &IrontideBtBackend, tmp: &std::path::Path) -> Id20 {
    let path = write_torrent_fixture(tmp);
    backend
        .start(StartDownloadRequest {
            url: path.to_string_lossy().into_owned(),
            ..Default::default()
        })
        .await
        .expect("start fixture torrent")
}

#[tokio::test]
async fn preview_torrent_rejects_magnet_links() {
    let (_tmp, backend) = make_backend().await;

    let err = backend
        .preview_torrent("magnet:?xt=urn:btih:aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d")
        .await
        .expect_err("magnet links cannot be previewed");
    assert!(
        matches!(err, DownloadError::TorrentInvalidData(_)),
        "expected TorrentInvalidData, got {err:?}"
    );

    backend.shutdown().await;
}

#[tokio::test]
async fn preview_torrent_reads_a_local_torrent_file() {
    let (tmp, backend) = make_backend().await;
    let path = write_torrent_fixture(tmp.path());

    let entries = backend
        .preview_torrent(&path.to_string_lossy())
        .await
        .expect("fixture must parse");
    assert_eq!(entries.len(), 2, "fixture has two files");
    assert_eq!(entries[0].index, 0);
    assert_eq!(entries[0].path, "a.txt");
    assert_eq!(entries[0].size, 10);
    assert_eq!(entries[1].index, 1);
    assert_eq!(entries[1].path, "b.txt");
    assert_eq!(entries[1].size, 20);

    backend.shutdown().await;
}

#[tokio::test]
async fn preview_torrent_reports_unreadable_and_malformed_files() {
    let (tmp, backend) = make_backend().await;

    let missing = tmp.path().join("does-not-exist.torrent");
    let err = backend
        .preview_torrent(&missing.to_string_lossy())
        .await
        .expect_err("missing file must fail");
    assert!(
        matches!(err, DownloadError::TorrentIo(_)),
        "expected TorrentIo for a missing file, got {err:?}"
    );

    let garbage = tmp.path().join("garbage.torrent");
    std::fs::write(&garbage, b"this is not bencode").expect("write garbage");
    let err = backend
        .preview_torrent(&garbage.to_string_lossy())
        .await
        .expect_err("garbage must fail");
    assert!(
        matches!(err, DownloadError::TorrentInvalidData(_)),
        "expected TorrentInvalidData for garbage, got {err:?}"
    );

    backend.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn queries_error_for_an_unknown_info_hash() {
    let (_tmp, backend) = make_backend().await;
    let unknown = Id20::from([0u8; 20]);

    assert!(backend.get_peers(unknown).is_err(), "peers must fail");
    assert!(backend.get_trackers(unknown).is_err(), "trackers must fail");
    assert!(backend.get_pieces(unknown).is_err(), "pieces must fail");
    assert!(
        backend.get_torrent_files(unknown).is_err(),
        "files must fail"
    );
    assert!(
        backend.update_torrent_files(unknown, vec![0]).await.is_err(),
        "file selection must fail"
    );

    backend.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn queries_work_for_a_metadata_less_magnet() {
    let (_tmp, backend) = make_backend().await;
    let info_hash = backend
        .start(magnet_request("no-metadata"))
        .await
        .expect("start magnet");

    // Without metadata the engine reports no peers/trackers/pieces and an
    // empty file list — but the calls must succeed instead of erroring.
    assert!(
        backend.get_peers(info_hash).expect("peers").is_empty(),
        "no peers in an offline session"
    );
    assert!(
        backend.get_trackers(info_hash).expect("trackers").is_empty(),
        "magnet has no announce list"
    );
    assert!(
        backend.get_pieces(info_hash).expect("pieces").is_empty(),
        "no pieces before metadata resolves"
    );
    assert!(
        backend.get_torrent_files(info_hash).expect("files").is_empty(),
        "no file list before metadata resolves"
    );

    // File selection needs metadata.
    let err = backend
        .update_torrent_files(info_hash, vec![0])
        .await
        .expect_err("selection must wait for metadata");
    assert!(
        matches!(err, DownloadError::TorrentInvalidData(_)),
        "expected TorrentInvalidData, got {err:?}"
    );

    let status = backend.runtime_status();
    assert!(!status.dht_enabled, "make_backend disables DHT");
    assert_eq!(status.torrent_count, 1);
    assert!(!status.connected, "offline session has no peers or DHT nodes");

    backend.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn queries_use_metadata_when_the_torrent_has_it() {
    let (tmp, backend) = make_backend().await;
    let info_hash = start_fixture(&backend, tmp.path()).await;

    let pieces = backend.get_pieces(info_hash).expect("pieces");
    assert_eq!(pieces.len(), 1, "30 bytes behind a 256-byte piece");
    assert!(!pieces[0].completed, "nothing downloaded yet");

    let files = backend.get_torrent_files(info_hash).expect("files");
    assert_eq!(files.len(), 2, "fixture has two files");
    assert_eq!(files[0].path, "a.txt");
    assert_eq!(files[0].size, 10);
    assert_eq!(files[0].downloaded_bytes, 0);
    assert_eq!(files[1].path, "b.txt");
    assert_eq!(files[1].size, 20);
    assert_eq!(files[1].downloaded_bytes, 0);
    // `included` is the selection state, not the disk open/closed mode:
    // a freshly started torrent keeps every file selected.
    assert!(
        files.iter().all(|f| f.included),
        "freshly started files are included: {files:?}"
    );

    // Pausing closes the disk handles, but must not deselect the files —
    // `included` is derived from file priorities, so the selection survives a
    // run-state change.
    backend.pause(info_hash).await.expect("pause");
    let paused_files = backend.get_torrent_files(info_hash).expect("files");
    assert!(
        paused_files.iter().all(|f| f.included),
        "a paused torrent keeps its selection: {paused_files:?}"
    );

    backend.shutdown().await;
}

/// A single-file torrent has no `files` list; the one entry must be
/// synthesized from `name`/`length` (mirroring the preview path), and
/// selection must still flip it.
#[tokio::test(flavor = "multi_thread")]
async fn single_file_torrent_reports_one_selectable_entry() {
    let (tmp, backend) = make_backend().await;
    let path = write_single_file_torrent_fixture(tmp.path());
    let info_hash = backend
        .start(StartDownloadRequest {
            url: path.to_string_lossy().into_owned(),
            ..Default::default()
        })
        .await
        .expect("start single-file fixture");

    let files = backend.get_torrent_files(info_hash).expect("files");
    assert_eq!(
        files.len(),
        1,
        "a single-file torrent reports exactly one entry: {files:?}"
    );
    assert_eq!(files[0].index, 0);
    assert_eq!(files[0].path, "single.bin");
    assert_eq!(files[0].size, 30);
    assert_eq!(files[0].downloaded_bytes, 0);
    assert!(files[0].included, "a fresh file is selected: {files:?}");

    backend
        .update_torrent_files(info_hash, vec![])
        .await
        .expect("deselect the only file");
    let files = backend.get_torrent_files(info_hash).expect("files");
    assert!(
        !files[0].included,
        "selection must apply to single-file torrents: {files:?}"
    );

    backend.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn update_torrent_files_sets_skip_priorities() {
    use irontide::core::FilePriority;

    let (tmp, backend) = make_backend().await;
    let info_hash = start_fixture(&backend, tmp.path()).await;

    backend
        .update_torrent_files(info_hash, vec![0])
        .await
        .expect("select file 0");
    let priorities = backend
        .session
        .file_priorities(info_hash)
        .await
        .expect("read priorities");
    assert_eq!(
        priorities,
        vec![FilePriority::Normal, FilePriority::Skip],
        "only the selected file stays normal"
    );
    // The aria2 `selected` flag / inspector checkbox reads the same source.
    let files = backend.get_torrent_files(info_hash).expect("files");
    assert_eq!(
        files.iter().map(|f| f.included).collect::<Vec<_>>(),
        vec![true, false],
        "included must follow the file priorities: {files:?}"
    );

    backend
        .update_torrent_files(info_hash, vec![])
        .await
        .expect("select nothing");
    let priorities = backend
        .session
        .file_priorities(info_hash)
        .await
        .expect("read priorities");
    assert_eq!(
        priorities,
        vec![FilePriority::Skip, FilePriority::Skip],
        "an empty selection skips every file"
    );

    backend.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn set_speed_limit_accepts_and_clears_both_directions() {
    let (_tmp, backend) = make_backend().await;
    let info_hash = backend
        .start(magnet_request("speed-limit"))
        .await
        .expect("start magnet");

    // Setting then clearing both directions must be a no-op from the caller's
    // perspective (the engine's per-torrent limits are applied internally).
    backend.set_speed_limit(info_hash, Some(1024), Some(2048));
    backend.set_speed_limit(info_hash, None, None);

    // Partial limits hit the other two branches.
    backend.set_speed_limit(info_hash, Some(512), None);
    backend.set_speed_limit(info_hash, None, Some(256));

    let list = backend.list().await.expect("list");
    assert_eq!(list.len(), 1, "torrent survives the limit churn");

    backend.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn emit_pending_summary_falls_back_without_stats() {
    let (_tmp, backend) = make_backend().await;
    let unknown = Id20::from([0xAB; 20]);
    let mut rx = backend.event_bus.subscribe();

    backend.emit_pending_summary(unknown);

    let event = rx.try_recv().expect("pending summary event");
    match event {
        DownloadEvent::Updated { id, summary_json } => {
            assert_eq!(id, unknown.to_hex());
            assert_eq!(summary_json["state"], "queued");
            assert_eq!(summary_json["fileName"], "Pending torrent");
            assert_eq!(summary_json["downloadedBytes"], 0);
        }
        other => panic!("unexpected event: {other:?}"),
    }

    backend.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn emit_pending_summary_uses_stats_when_available() {
    let (tmp, backend) = make_backend().await;
    let info_hash = start_fixture(&backend, tmp.path()).await;
    let mut rx = backend.event_bus.subscribe();

    backend.emit_pending_summary(info_hash);

    let event = rx.try_recv().expect("pending summary event");
    match event {
        DownloadEvent::Updated { id, summary_json } => {
            assert_eq!(id, info_hash.to_hex());
            assert_eq!(summary_json["id"], info_hash.to_hex());
            assert_ne!(
                summary_json["fileName"], "Pending torrent",
                "stats are available, so the fallback must not be used"
            );
        }
        other => panic!("unexpected event: {other:?}"),
    }

    backend.shutdown().await;
}
