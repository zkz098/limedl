//! Tests for the persisted BitTorrent task index (`bt_task_repo`).

use super::Database;
use crate::types::{BtUploadStatus, DownloadState, DownloadSummary, Priority, TaskKind, ThreadMode};

fn summary(id: &str, state: DownloadState, created_at_ms: u64) -> DownloadSummary {
    DownloadSummary {
        id: id.to_string(),
        kind: TaskKind::Bt,
        state,
        url: id.to_string(),
        file_name: format!("{id}.bin"),
        destination_path: "/tmp/bt".into(),
        total_bytes: Some(4096),
        downloaded_bytes: 2048,
        connection_count: 3,
        thread_mode: ThreadMode::Fixed,
        requested_thread_count: None,
        desired_thread_count: None,
        allocated_thread_count: None,
        adaptive_profile: None,
        thread_note: None,
        speed_bytes_per_second: Some(1234.0),
        eta_seconds: Some(9),
        uploaded_bytes: Some(512),
        upload_speed_bytes_per_second: Some(10.0),
        peer_count: Some(3),
        upload_status: Some(BtUploadStatus::Uploading),
        info_hash: Some(id.to_string()),
        expected_checksum: None,
        error: None,
        cdn_accelerated: false,
        cdn_node_ip: None,
        created_at_ms,
        priority: Priority::Normal,
        seed_count: Some(1),
        leech_count: Some(2),
        download_limit_bps: None,
        upload_limit_bps: None,
        chunks: Vec::new(),
        mirror_url: None,
    }
}

#[test]
fn replace_then_list_round_trips_every_field() {
    let db = Database::open_in_memory().unwrap();
    db.replace_bt_tasks(&[summary("aa", DownloadState::Downloading, 100)])
        .unwrap();

    let tasks = db.list_bt_tasks().unwrap();
    assert_eq!(tasks.len(), 1);
    let task = &tasks[0];
    assert_eq!(task.id, "aa");
    assert_eq!(task.state, DownloadState::Downloading);
    assert_eq!(task.file_name, "aa.bin");
    assert_eq!(task.total_bytes, Some(4096));
    assert_eq!(task.downloaded_bytes, 2048);
    assert_eq!(task.speed_bytes_per_second, Some(1234.0));
    assert_eq!(task.upload_status, Some(BtUploadStatus::Uploading));
}

#[test]
fn replace_is_a_full_snapshot_not_an_upsert() {
    let db = Database::open_in_memory().unwrap();
    db.replace_bt_tasks(&[
        summary("aa", DownloadState::Paused, 100),
        summary("bb", DownloadState::Completed, 200),
    ])
    .unwrap();

    // A later engine view without "aa" must drop it, not merge.
    db.replace_bt_tasks(&[summary("bb", DownloadState::Completed, 200)])
        .unwrap();

    let ids: Vec<String> = db.list_bt_tasks().unwrap().into_iter().map(|t| t.id).collect();
    assert_eq!(ids, vec!["bb".to_string()]);
}

#[test]
fn list_orders_newest_first() {
    let db = Database::open_in_memory().unwrap();
    db.replace_bt_tasks(&[
        summary("old", DownloadState::Paused, 100),
        summary("new", DownloadState::Paused, 300),
        summary("mid", DownloadState::Paused, 200),
    ])
    .unwrap();

    let ids: Vec<String> = db.list_bt_tasks().unwrap().into_iter().map(|t| t.id).collect();
    assert_eq!(ids, vec!["new", "mid", "old"]);
}

#[test]
fn delete_removes_only_the_target_row() {
    let db = Database::open_in_memory().unwrap();
    db.replace_bt_tasks(&[
        summary("aa", DownloadState::Paused, 100),
        summary("bb", DownloadState::Paused, 200),
    ])
    .unwrap();

    db.delete_bt_task("aa").unwrap();

    let ids: Vec<String> = db.list_bt_tasks().unwrap().into_iter().map(|t| t.id).collect();
    assert_eq!(ids, vec!["bb".to_string()]);

    // Deleting a missing row is a no-op, not an error.
    db.delete_bt_task("missing").unwrap();
}

#[test]
fn corrupt_json_rows_are_skipped_not_fatal() {
    let db = Database::open_in_memory().unwrap();
    db.replace_bt_tasks(&[summary("good", DownloadState::Paused, 100)])
        .unwrap();
    {
        let conn = db.lock_write();
        conn.execute(
            "INSERT INTO bt_tasks (id, summary_json, created_at_ms) VALUES ('bad', '{not json', 50)",
            [],
        )
        .unwrap();
    }

    let tasks = db.list_bt_tasks().unwrap();
    assert_eq!(tasks.len(), 1, "the readable row must survive");
    assert_eq!(tasks[0].id, "good");
}
