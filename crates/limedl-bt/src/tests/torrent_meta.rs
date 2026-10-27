//! Torrent metadata and file-entry previews.

use super::*;

#[tokio::test]
async fn test_preview_single_file_torrent() {
    let (tmp, backend) = make_backend().await;
    let path = write_single_file_torrent_fixture(tmp.path());
    let entries = backend
        .preview_torrent(&path.to_string_lossy())
        .await
        .expect("preview");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].index, 0);
    assert_eq!(entries[0].path, "single.bin");
    assert_eq!(entries[0].size, 30);
    backend.shutdown().await;
}

#[tokio::test]
async fn test_preview_multi_file_torrent() {
    let (tmp, backend) = make_backend().await;
    let path = write_torrent_fixture(tmp.path());
    let entries = backend
        .preview_torrent(&path.to_string_lossy())
        .await
        .expect("preview");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].index, 0);
    assert_eq!(entries[0].path, "a.txt");
    assert_eq!(entries[0].size, 10);
    assert_eq!(entries[1].index, 1);
    assert_eq!(entries[1].path, "b.txt");
    assert_eq!(entries[1].size, 20);
    backend.shutdown().await;
}
