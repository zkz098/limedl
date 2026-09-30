//! DownloadBackend trait surface.

use super::*;

#[tokio::test]
async fn backend_new_and_shutdown() {
    let (_tmp, backend) = make_backend().await;
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_list_empty_on_start() {
    let (_tmp, backend) = make_backend().await;
    let list: Vec<DownloadSummary> = backend.list().await.expect("list should succeed");
    assert!(list.is_empty(), "expected empty list on fresh backend");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_start_returns_error_for_invalid_magnet() {
    let (_tmp, backend) = make_backend().await;
    let request = StartDownloadRequest {
        url: "magnet:?xt=urn:btih:invalid&dn=test".into(),
        ..Default::default()
    };
    let result: Result<TaskId, _> = DownloadBackend::start(&backend, request).await;
    assert!(result.is_err(), "expected error for invalid magnet link");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_start_returns_error_for_empty_url() {
    let (_tmp, backend) = make_backend().await;
    let request = StartDownloadRequest::default();
    let result: Result<TaskId, _> = DownloadBackend::start(&backend, request).await;
    assert!(result.is_err(), "expected error for empty URL");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_status_returns_not_found_for_unknown_id() {
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Bt(Id20::from([0u8; 20]));
    let result = DownloadBackend::status(&backend, &fake_id).await;
    assert!(result.is_err(), "expected error for unknown info hash");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_status_returns_not_found_for_wrong_variant() {
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Http(uuid::Uuid::nil());
    let result = DownloadBackend::status(&backend, &fake_id).await;
    assert!(result.is_err(), "expected error for wrong TaskId variant");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_pause_returns_not_found_for_unknown_id() {
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Bt(Id20::from([0u8; 20]));
    let result = DownloadBackend::pause(&backend, &fake_id).await;
    assert!(result.is_err(), "expected error for unknown info hash");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_pause_returns_not_found_for_wrong_variant() {
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Http(uuid::Uuid::nil());
    let result = DownloadBackend::pause(&backend, &fake_id).await;
    assert!(result.is_err(), "expected error for wrong TaskId variant");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_resume_returns_not_found_for_unknown_id() {
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Bt(Id20::from([0u8; 20]));
    let result = DownloadBackend::resume(&backend, &fake_id).await;
    assert!(result.is_err(), "expected error for unknown info hash");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_resume_returns_not_found_for_wrong_variant() {
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Http(uuid::Uuid::nil());
    let result = DownloadBackend::resume(&backend, &fake_id).await;
    assert!(result.is_err(), "expected error for wrong TaskId variant");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_cancel_returns_ok_for_unknown_id() {
    // Cancel is tolerant: always succeeds even for unknown info hashes,
    // returning a fallback snapshot with Canceled state.
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Bt(Id20::from([0u8; 20]));
    let result = DownloadBackend::cancel(&backend, &fake_id).await;
    assert!(
        result.is_ok(),
        "cancel should succeed even for unknown hash"
    );
    let snapshot = result.unwrap();
    assert_eq!(
        snapshot.state,
        DownloadState::Canceled,
        "should return Canceled state for unknown hash"
    );
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_cancel_returns_not_found_for_wrong_variant() {
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Http(uuid::Uuid::nil());
    let result = DownloadBackend::cancel(&backend, &fake_id).await;
    assert!(result.is_err(), "expected error for wrong TaskId variant");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_remove_returns_not_found_for_unknown_id() {
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Bt(Id20::from([0u8; 20]));
    let result = DownloadBackend::remove(&backend, &fake_id).await;
    assert!(result.is_err(), "expected error for unknown info hash");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_remove_returns_not_found_for_wrong_variant() {
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Http(uuid::Uuid::nil());
    let result = DownloadBackend::remove(&backend, &fake_id).await;
    assert!(result.is_err(), "expected error for wrong TaskId variant");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_purge_returns_not_found_for_unknown_id() {
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Bt(Id20::from([0u8; 20]));
    let result = DownloadBackend::purge(&backend, &fake_id).await;
    assert!(result.is_err(), "expected error for unknown info hash");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_purge_returns_not_found_for_wrong_variant() {
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Http(uuid::Uuid::nil());
    let result = DownloadBackend::purge(&backend, &fake_id).await;
    assert!(result.is_err(), "expected error for wrong TaskId variant");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_update_settings_does_not_panic() {
    let (_tmp, backend) = make_backend().await;
    let settings = AppSettings::default();
    let result = DownloadBackend::update_settings(&backend, &settings).await;
    assert!(result.is_ok(), "update_settings should succeed");
    backend.shutdown().await;
}

#[ignore]
#[tokio::test]
async fn backend_start_magnet_and_poll_status() {
    let (_tmp, backend) = make_network_backend().await;

    let request = StartDownloadRequest {
        url: "magnet:?xt=urn:btih:aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d&dn=test".into(),
        ..Default::default()
    };
    let task_id = DownloadBackend::start(&backend, request)
        .await
        .expect("start should succeed for valid magnet");

    // Verify it's listed
    let list = backend.list().await.expect("list should succeed");
    assert_eq!(list.len(), 1, "expected 1 torrent in list");
    assert_eq!(list[0].id, task_id.raw_id(), "task id should match");

    // Get status
    let status = DownloadBackend::status(&backend, &task_id)
        .await
        .expect("status should succeed for started magnet");
    assert_eq!(status.id, task_id.raw_id());
    assert_eq!(status.kind, TaskKind::Bt);

    backend.shutdown().await;
}

#[ignore]
#[tokio::test]
async fn backend_pause_resume_active_download() {
    let (_tmp, backend) = make_network_backend().await;

    let request = StartDownloadRequest {
        url: "magnet:?xt=urn:btih:aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d&dn=test".into(),
        ..Default::default()
    };
    let task_id = DownloadBackend::start(&backend, request)
        .await
        .expect("start should succeed");

    // Pause
    let paused = DownloadBackend::pause(&backend, &task_id)
        .await
        .expect("pause should succeed");
    assert_eq!(paused.state, DownloadState::Paused, "should be paused");

    // Resume
    let resumed = DownloadBackend::resume(&backend, &task_id)
        .await
        .expect("resume should succeed");
    assert_ne!(
        resumed.state,
        DownloadState::Paused,
        "should no longer be paused after resume"
    );

    backend.shutdown().await;
}

#[ignore]
#[tokio::test]
async fn backend_cancel_active_download() {
    let (_tmp, backend) = make_network_backend().await;

    let request = StartDownloadRequest {
        url: "magnet:?xt=urn:btih:aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d&dn=test".into(),
        ..Default::default()
    };
    let task_id = DownloadBackend::start(&backend, request)
        .await
        .expect("start should succeed");

    // Cancel
    let canceled = DownloadBackend::cancel(&backend, &task_id)
        .await
        .expect("cancel should succeed");
    assert_eq!(
        canceled.state,
        DownloadState::Canceled,
        "should be canceled"
    );

    backend.shutdown().await;
}

#[ignore]
#[tokio::test]
async fn backend_shutdown_while_active() {
    let (_tmp, backend) = make_network_backend().await;

    let request = StartDownloadRequest {
        url: "magnet:?xt=urn:btih:aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d&dn=test".into(),
        ..Default::default()
    };
    let _task_id = DownloadBackend::start(&backend, request)
        .await
        .expect("start should succeed");

    // Should not panic
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_open_in_explorer_returns_not_found_for_wrong_variant() {
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Http(uuid::Uuid::nil());
    let result = DownloadBackend::open_in_explorer(&backend, &fake_id).await;
    assert!(result.is_err(), "expected error for wrong TaskId variant");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_open_in_explorer_returns_error_for_unknown_id() {
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Bt(Id20::from([0u8; 20]));
    let result = DownloadBackend::open_in_explorer(&backend, &fake_id).await;
    assert!(result.is_err(), "expected error for unknown info hash");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_double_shutdown_does_not_panic() {
    let (_tmp, backend) = make_backend().await;
    backend.shutdown().await;
    backend.shutdown().await; // second call must not panic
}

#[tokio::test(flavor = "multi_thread")]
async fn backend_pause_twice_no_panic() {
    let (_tmp, backend) = make_backend().await;

    let request = StartDownloadRequest {
        url: "magnet:?xt=urn:btih:1111111111111111111111111111111111111111&dn=pause-twice".into(),
        ..Default::default()
    };
    let task_id = DownloadBackend::start(&backend, request)
        .await
        .expect("start should succeed for valid magnet");

    // First pause
    let paused = DownloadBackend::pause(&backend, &task_id).await;
    assert!(paused.is_ok(), "first pause should succeed");

    // Second pause should not panic
    let paused2 = DownloadBackend::pause(&backend, &task_id).await;
    assert!(paused2.is_ok(), "second pause should also succeed");

    backend.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn backend_resume_active_no_panic() {
    let (_tmp, backend) = make_backend().await;

    let request = StartDownloadRequest {
        url: "magnet:?xt=urn:btih:2222222222222222222222222222222222222222&dn=resume-active".into(),
        ..Default::default()
    };
    let task_id = DownloadBackend::start(&backend, request)
        .await
        .expect("start should succeed for valid magnet");

    // Resume while already active (not paused) should not panic
    let resumed = DownloadBackend::resume(&backend, &task_id).await;
    assert!(resumed.is_ok(), "resume-active should not panic");

    backend.shutdown().await;
}
