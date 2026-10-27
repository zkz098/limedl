//! Librqbit event bridge: info-hash extraction and event mapping.

use super::*;
use crate::event_bus::DownloadEvent;

#[test]
fn test_extract_info_hash_variants() {
    let ih = Id20::new([1u8; 20]);
    assert_eq!(extract_info_hash(&BtEvent::Added(ih)), Some(&ih));
    assert_eq!(extract_info_hash(&BtEvent::Paused(ih)), Some(&ih));
    assert_eq!(extract_info_hash(&BtEvent::Resumed(ih)), Some(&ih));
    assert_eq!(extract_info_hash(&BtEvent::Finished(ih)), Some(&ih));
    assert_eq!(
        extract_info_hash(&BtEvent::Error(ih, "err".into())),
        Some(&ih)
    );
}

#[tokio::test]
async fn test_handle_event_added() {
    let (_tmp, backend) = make_backend().await;
    let mut rx = backend.event_bus.subscribe();
    let ih = Id20::new([1u8; 20]);

    backend.handle_event(&BtEvent::Added(ih)).await;

    let event = rx.try_recv().expect("event");
    match event {
        DownloadEvent::Aria2Notification { event_name, gid } => {
            assert_eq!(event_name, "aria2.onDownloadStart");
            assert_eq!(gid, internal_id_to_gid(&ih));
        }
        other => panic!("unexpected event: {other:?}"),
    }
    backend.shutdown().await;
}

#[tokio::test]
async fn test_handle_event_paused() {
    let (_tmp, backend) = make_backend().await;
    let mut rx = backend.event_bus.subscribe();
    let ih = Id20::new([2u8; 20]);

    backend.handle_event(&BtEvent::Paused(ih)).await;

    let event = rx.try_recv().expect("event");
    match event {
        DownloadEvent::Aria2Notification { event_name, gid } => {
            assert_eq!(event_name, "aria2.onDownloadPause");
            assert_eq!(gid, internal_id_to_gid(&ih));
        }
        other => panic!("unexpected event: {other:?}"),
    }
    backend.shutdown().await;
}

#[tokio::test]
async fn test_handle_event_finished() {
    let (_tmp, backend) = make_backend().await;
    let mut rx = backend.event_bus.subscribe();
    let ih = Id20::new([3u8; 20]);

    backend.handle_event(&BtEvent::Finished(ih)).await;

    let event1 = rx.try_recv().expect("event1");
    match event1 {
        DownloadEvent::Aria2Notification { event_name, .. } => {
            assert_eq!(event_name, "aria2.onDownloadComplete");
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let event2 = rx.try_recv().expect("event2");
    match event2 {
        DownloadEvent::Aria2Notification { event_name, .. } => {
            assert_eq!(event_name, "aria2.onBtDownloadComplete");
        }
        other => panic!("unexpected event: {other:?}"),
    }
    backend.shutdown().await;
}

#[tokio::test]
async fn test_handle_event_error() {
    let (_tmp, backend) = make_backend().await;
    let mut rx = backend.event_bus.subscribe();
    let ih = Id20::new([4u8; 20]);

    backend
        .handle_event(&BtEvent::Error(ih, "fail".into()))
        .await;

    let event = rx.try_recv().expect("event");
    match event {
        DownloadEvent::Aria2Notification { event_name, .. } => {
            assert_eq!(event_name, "aria2.onDownloadError");
        }
        other => panic!("unexpected event: {other:?}"),
    }
    backend.shutdown().await;
}
