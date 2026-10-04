//! Concurrent read/write behaviour.

use super::*;

#[timeout(30_000)]
#[test]
fn concurrent_insert_and_read() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = Arc::new(Database::open(&path).unwrap());

    let db_reader = db.clone();
    let reader = std::thread::spawn(move || {
        for i in 0..100 {
            let _ = db_reader.get_download("concurrent-1");
            let _ = db_reader.count_downloads();
            if i % 10 == 0 {
                std::thread::yield_now();
            }
        }
        let _ = db_reader.get_download("concurrent-1");
    });

    let db_writer = db.clone();
    let writer = std::thread::spawn(move || {
        let manifest = new_test_manifest("concurrent-1", "https://example.com/con", "con.bin");
        db_writer.insert_download(&manifest).unwrap();
    });

    reader.join().expect("reader thread panicked");
    writer.join().expect("writer thread panicked");

    let loaded = db
        .get_download("concurrent-1")
        .unwrap()
        .expect("should exist after concurrent access");
    assert_eq!(loaded.url, "https://example.com/con");
    assert_eq!(db.count_downloads().unwrap(), 1);
}

#[timeout(30_000)]
#[test]
fn concurrent_multiple_writes_different_ids() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = Arc::new(Database::open(&path).unwrap());

    let mut handles = Vec::new();
    for i in 0..5 {
        let db_clone = db.clone();
        let id = format!("con-write-{i}");
        handles.push(std::thread::spawn(move || {
            let manifest = new_test_manifest(
                &id,
                &format!("https://example.com/file{i}"),
                &format!("file{i}.bin"),
            );
            db_clone.insert_download(&manifest).unwrap();
        }));
    }

    for handle in handles {
        handle.join().expect("writer thread panicked");
    }

    assert_eq!(db.count_downloads().unwrap(), 5);
    for i in 0..5 {
        let id = format!("con-write-{i}");
        let loaded = db
            .get_download(&id)
            .unwrap()
            .unwrap_or_else(|| panic!("download {id} should exist"));
        assert_eq!(loaded.id, id);
        assert_eq!(loaded.file_name, format!("file{i}.bin"));
        assert_eq!(loaded.url, format!("https://example.com/file{i}"));
    }
}

#[timeout(30_000)]
#[test]
fn concurrent_read_write_same_id() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = Arc::new(Database::open(&path).unwrap());

    {
        let initial = new_test_manifest("same-id", "https://example.com/initial", "initial.bin");
        db.insert_download(&initial).unwrap();
    }

    let db_writer = db.clone();
    let writer = std::thread::spawn(move || {
        for i in 0..15 {
            let mut manifest =
                new_test_manifest("same-id", "https://example.com/updated", "updated.bin");
            manifest.downloaded_bytes = (i as u64 + 1) * 100;
            manifest.updated_at_ms = i as u64;
            manifest.state = DownloadState::Downloading;
            db_writer.update_download(&manifest).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(3));
        }
    });

    let db_reader = db.clone();
    let reader = std::thread::spawn(move || {
        for _ in 0..30 {
            let result = db_reader.get_download("same-id");
            assert!(
                result.is_ok(),
                "reader should not encounter DB errors during concurrent access"
            );
            if let Ok(Some(manifest)) = result {
                assert_eq!(manifest.id, "same-id");
                assert!(
                    manifest.downloaded_bytes <= 1500,
                    "downloaded_bytes should be bounded: got {}",
                    manifest.downloaded_bytes
                );
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    });

    writer.join().expect("writer thread panicked");
    reader.join().expect("reader thread panicked");

    let loaded = db
        .get_download("same-id")
        .unwrap()
        .expect("should exist after concurrent rw");
    assert_eq!(loaded.downloaded_bytes, 1500);
    assert_eq!(loaded.updated_at_ms, 14);
    assert_eq!(loaded.state, DownloadState::Downloading);
}

#[timeout(30_000)]
#[test]
fn concurrent_load_chunks_while_saving() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = Arc::new(Database::open(&path).unwrap());

    {
        let mut manifest = new_test_manifest("chunks-con", "https://example.com/cc", "cc.bin");
        manifest.chunks = vec![
            ChunkManifest {
                index: 0,
                start: 0,
                end: 499,
                downloaded: 0,
                durable_downloaded: 0,
                completed: false,
                claimed_by: None,
                dirty: false,
            },
            ChunkManifest {
                index: 1,
                start: 500,
                end: 999,
                downloaded: 0,
                durable_downloaded: 0,
                completed: false,
                claimed_by: None,
                dirty: false,
            },
        ];
        db.insert_download(&manifest).unwrap();
    }

    let db_writer = db.clone();
    let writer = std::thread::spawn(move || {
        for i in 0..20 {
            let progress = (i as u64 + 1) * 50;
            let mut manifest = new_test_manifest("chunks-con", "https://example.com/cc", "cc.bin");
            manifest.downloaded_bytes = progress * 2;
            manifest.chunks = vec![
                ChunkManifest {
                    index: 0,
                    start: 0,
                    end: 499,
                    downloaded: progress.min(500),
                    durable_downloaded: progress.min(500),
                    completed: progress >= 500,
                    claimed_by: None,
                    dirty: false,
                },
                ChunkManifest {
                    index: 1,
                    start: 500,
                    end: 999,
                    downloaded: progress.saturating_sub(500),
                    durable_downloaded: progress.saturating_sub(500),
                    completed: progress >= 1000,
                    claimed_by: None,
                    dirty: false,
                },
            ];
            db_writer.update_download(&manifest).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    });

    let db_reader = db.clone();
    let reader = std::thread::spawn(move || {
        for _ in 0..20 {
            let result = db_reader.get_download("chunks-con");
            assert!(
                result.is_ok(),
                "get_download should not error during concurrent chunk writes"
            );
            if let Ok(Some(manifest)) = &result {
                for chunk in &manifest.chunks {
                    assert!(
                        chunk.downloaded <= chunk.end - chunk.start,
                        "chunk {} download {} exceeds range {}-{}",
                        chunk.index,
                        chunk.downloaded,
                        chunk.start,
                        chunk.end
                    );
                }
            }
            let chunks = db_reader.load_chunks("chunks-con");
            assert!(
                chunks.is_ok(),
                "load_chunks should not error during concurrent chunk writes"
            );
            if let Ok(chunks) = &chunks {
                for chunk in chunks {
                    assert!(
                        chunk.downloaded <= chunk.end - chunk.start,
                        "chunk {} download {} exceeds range {}-{}",
                        chunk.index,
                        chunk.downloaded,
                        chunk.start,
                        chunk.end
                    );
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    });

    writer.join().expect("writer thread panicked");
    reader.join().expect("reader thread panicked");

    let loaded = db
        .get_download("chunks-con")
        .unwrap()
        .expect("should exist after concurrent chunk access");
    assert_eq!(loaded.downloaded_bytes, 2000);
    assert_eq!(loaded.chunks.len(), 2);
    assert!(loaded.chunks[0].completed);
    assert_eq!(loaded.chunks[0].downloaded, 500);
    assert!(loaded.chunks[1].completed);
    assert_eq!(loaded.chunks[1].downloaded, 500);
}
