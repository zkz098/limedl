//! Upload policy loop: limit tracking, cleanup and error paths.
//!
//! The pause branch itself needs real uploaded bytes, which an offline session
//! cannot produce — these tests cover the sweep, the clear-limits cleanup and
//! the stats-error path around it.

use super::*;

use crate::event_bus::DownloadEvent;

#[tokio::test]
async fn upload_policy_unpauses_when_limits_are_cleared() {
    let (_tmp, backend) = make_backend().await;
    {
        let mut settings = backend.bt_settings.lock();
        settings.upload_limit_bytes = 0;
        settings.upload_ratio_limit = 0.0;
    }
    let info_hash = Id20::from([0x11; 20]);
    backend.paused_by_limit.insert(info_hash, ());
    let mut rx = backend.event_bus.subscribe();

    let backend = Arc::new(backend);
    Arc::clone(&backend).spawn_upload_policy_loop();

    // The first interval tick fires immediately: with both limits cleared the
    // loop must un-cap any previously paused torrents and say so.
    let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("unpause event within timeout")
        .expect("event bus stays open");
    match event {
        DownloadEvent::Updated { id, summary_json } => {
            assert_eq!(id, info_hash.to_hex());
            assert_eq!(summary_json["uploadStatus"], "idle");
        }
        other => panic!("unexpected event: {other:?}"),
    }
    assert!(
        backend.paused_by_limit.is_empty(),
        "clearing the limits must clear the paused set"
    );

    backend.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn upload_policy_sweeps_active_torrents_without_hitting_the_limit() {
    let (tmp, backend) = make_backend().await;
    {
        let mut settings = backend.bt_settings.lock();
        settings.upload_limit_bytes = 1_000_000;
        settings.upload_ratio_limit = 0.5;
        settings.pause_upload_when_limit_reached = true;
    }
    let path = write_torrent_fixture(tmp.path());
    let info_hash = backend
        .start(StartDownloadRequest {
            url: path.to_string_lossy().into_owned(),
            ..Default::default()
        })
        .await
        .expect("start fixture");
    let mut rx = backend.event_bus.subscribe();

    let backend = Arc::new(backend);
    Arc::clone(&backend).spawn_upload_policy_loop();

    // The first tick reads real stats (Ok path) and computes both limit checks.
    // Nothing was uploaded, so the torrent must not be paused and no event may
    // be emitted; the loop then parks on its 5-second interval.
    assert!(
        tokio::time::timeout(Duration::from_millis(300), rx.recv())
            .await
            .is_err(),
        "a torrent below its limits must not be paused"
    );
    assert!(
        backend.paused_by_limit.is_empty(),
        "nothing was paused by the sweep"
    );
    assert!(
        backend.task_map.contains_key(&info_hash),
        "the sweep must not remove tasks"
    );

    backend.shutdown().await;
}

#[tokio::test]
async fn upload_policy_ignores_stats_errors_for_unknown_torrents() {
    let (_tmp, backend) = make_backend().await;
    {
        let mut settings = backend.bt_settings.lock();
        settings.upload_limit_bytes = 1024;
        settings.pause_upload_when_limit_reached = true;
    }
    // A task the engine does not know: `torrent_stats` fails and the sweep must
    // log-and-continue instead of pausing or panicking.
    let unknown = Id20::from([0x22; 20]);
    backend.task_map.insert(unknown, unknown);

    let backend = Arc::new(backend);
    Arc::clone(&backend).spawn_upload_policy_loop();
    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(backend.paused_by_limit.is_empty());

    backend.shutdown().await;
}

#[tokio::test]
async fn upload_policy_spawn_replaces_the_previous_loop() {
    let (_tmp, backend) = make_backend().await;
    let backend = Arc::new(backend);

    Arc::clone(&backend).spawn_upload_policy_loop();
    let first = backend.upload_policy_task.lock().as_ref().map(|h| h.id());
    assert!(first.is_some(), "spawn stores a join handle");

    Arc::clone(&backend).spawn_upload_policy_loop();
    let second = backend.upload_policy_task.lock().as_ref().map(|h| h.id());
    assert!(second.is_some(), "the replacement loop is stored");
    assert_ne!(first, second, "spawn must abort and replace the previous loop");

    backend.shutdown().await;
}
