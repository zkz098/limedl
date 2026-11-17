use super::super::chunk_repo::{decode_chunks, encode_chunks};
use super::super::schema::{ACTIVE_DOWNLOADS, TERMINAL_DOWNLOADS};
use super::*;
use crate::alt_svc::AltSvcRecord;

#[test]
fn active_and_terminal_tables_are_isolated() {
    let db = Database::open_in_memory().unwrap();

    let mut active = new_test_manifest("task-1", "https://example.com/1", "1.bin");
    active.state = DownloadState::Downloading;
    db.insert_download(&active).unwrap();

    {
        let tx = db.begin_read().unwrap();
        let a_tbl = tx.open_table(ACTIVE_DOWNLOADS).unwrap();
        let t_tbl = tx.open_table(TERMINAL_DOWNLOADS).unwrap();
        assert!(a_tbl.get("task-1").unwrap().is_some());
        assert!(t_tbl.get("task-1").unwrap().is_none());
    }
    assert_eq!(db.count_terminal_downloads().unwrap(), 0);

    // Transition to terminal moves the record out of ACTIVE_DOWNLOADS into TERMINAL_DOWNLOADS.
    active.state = DownloadState::Completed;
    active.downloaded_bytes = 1024;
    db.insert_download(&active).unwrap();

    {
        let tx = db.begin_read().unwrap();
        let a_tbl = tx.open_table(ACTIVE_DOWNLOADS).unwrap();
        let t_tbl = tx.open_table(TERMINAL_DOWNLOADS).unwrap();
        assert!(a_tbl.get("task-1").unwrap().is_none());
        assert!(t_tbl.get("task-1").unwrap().is_some());
    }
    assert_eq!(db.count_terminal_downloads().unwrap(), 1);
}

#[test]
fn compact_chunk_codec_roundtrip() {
    let chunks = vec![
        ChunkManifest {
            index: 0,
            start: 0,
            end: 4095,
            downloaded: 4096,
            durable_downloaded: 4096,
            completed: true,
            claimed_by: None,
            dirty: true,
        },
        ChunkManifest {
            index: 1,
            start: 4096,
            end: 8191,
            downloaded: 2048,
            durable_downloaded: 1024,
            completed: false,
            claimed_by: Some(3),
            dirty: true,
        },
    ];

    let encoded = encode_chunks(&chunks);
    let decoded = decode_chunks(&encoded).unwrap();
    assert_eq!(decoded.len(), 2);
    assert_eq!(decoded[0].downloaded, 4096);
    assert!(decoded[0].completed);
    assert_eq!(decoded[0].claimed_by, None);
    assert!(!decoded[0].dirty);

    // Durable progress (1024) is what survives persistence, not in-flight (2048).
    assert_eq!(decoded[1].downloaded, 1024);
    assert_eq!(decoded[1].durable_downloaded, 1024);
    assert!(!decoded[1].completed);
    assert_eq!(decoded[1].claimed_by, Some(3));
}

#[test]
fn corrupt_ephemeral_caches_reset_without_quarantining_core_db() {
    let dir = tempfile::tempdir().unwrap();
    let core_path = dir.path().join("downloads.redb");

    {
        let db = Database::open(&core_path).unwrap();
        db.insert_download(&new_test_manifest(
            "keep-me",
            "https://example.com/keep",
            "keep.bin",
        ))
        .unwrap();
        db.upsert_alt_svc(&AltSvcRecord {
            origin_host: "example.com".to_string(),
            origin_port: 443,
            protocol: "h3".to_string(),
            alt_host: "cdn.example.com".to_string(),
            alt_port: 443,
            expires_at_ms: 999_999_999,
            persist: true,
            fail_count: 0,
            broken_until_ms: 0,
        })
        .unwrap();
    }

    // Corrupt both ephemeral caches on disk; core downloads.redb remains intact.
    std::fs::write(dir.path().join("bt_tasks.redb"), b"corrupt bt cache").unwrap();
    std::fs::write(dir.path().join("alt_svc.redb"), b"corrupt alt_svc cache").unwrap();

    let db = Database::open(&core_path).unwrap();
    assert!(
        db.quarantined_path().is_none(),
        "ephemeral cache corruption must not quarantine core downloads.redb"
    );
    assert_eq!(db.count_downloads().unwrap(), 1);
    assert!(db.get_download("keep-me").unwrap().is_some());
    assert!(db.load_all_alt_svc().unwrap().is_empty());
}
