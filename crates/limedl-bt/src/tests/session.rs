//! Live librqbit session lifecycle.

use super::*;

#[tokio::test]
async fn test_session_create_and_shutdown() {
    let (_tmp, backend) = make_backend().await;
    let list = backend.list().await.unwrap();
    assert!(list.is_empty());
    backend.shutdown().await;
}

#[tokio::test]
async fn test_session_magnet_add_and_remove() {
    let (_tmp, backend) = make_backend().await;

    let info_hash = backend
        .start(StartDownloadRequest {
            url: "magnet:?xt=urn:btih:aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d&dn=test".into(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(
        info_hash.as_string(),
        "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d"
    );

    let list = backend.list().await.unwrap();
    assert_eq!(list.len(), 1);

    backend.remove(info_hash).await.unwrap();
    let list = backend.list().await.unwrap();
    assert!(list.is_empty());

    backend.shutdown().await;
}

#[tokio::test]
async fn test_session_pause_resume_magnet() {
    let (_tmp, backend) = make_backend().await;

    let info_hash = backend
        .start(StartDownloadRequest {
            url: "magnet:?xt=urn:btih:4a8eeb4c2f4f3ae1e2a8a3d4b5c6d7e8f9a0b1c2&dn=pause-test"
                .into(),
            ..Default::default()
        })
        .await
        .unwrap();

    let snap = backend.pause(info_hash).await.unwrap();
    assert_eq!(snap.state, DownloadState::Paused);

    let snap = backend.resume(info_hash).await.unwrap();
    assert_ne!(snap.state, DownloadState::Paused);

    backend.shutdown().await;
}
