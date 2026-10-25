//! Torrent metadata and file-entry previews.

use super::*;

#[test]
fn test_v1_file_entries_single_file() {
    let info = irontide::core::InfoDict {
        name: "ubuntu.iso".into(),
        piece_length: 262144,
        pieces: vec![0u8; 20],
        length: Some(1_000_000_000),
        files: None,
        private: None,
        source: None,
        ssl_cert: None,
        similar: vec![],
        collections: vec![],
    };
    let v1 = irontide::core::TorrentMetaV1 {
        info_hash: Id20::from([0u8; 20]),
        announce: None,
        announce_list: None,
        comment: None,
        created_by: None,
        creation_date: None,
        info,
        url_list: vec![],
        httpseeds: vec![],
        info_bytes: None,
        ssl_cert: None,
    };
    let entries = v1_file_entries(&v1);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].index, 0);
    assert_eq!(entries[0].path, "ubuntu.iso");
    assert_eq!(entries[0].size, 1_000_000_000);
}

#[test]
fn test_v1_file_entries_multi_file() {
    let files = vec![
        irontide::core::FileEntry {
            length: 500,
            path: vec!["dir".into(), "file1.txt".into()],
            attr: None,
            mtime: None,
            symlink_path: None,
        },
        irontide::core::FileEntry {
            length: 1200,
            path: vec!["file2.txt".into()],
            attr: None,
            mtime: None,
            symlink_path: None,
        },
    ];
    let info = irontide::core::InfoDict {
        name: "mydir".into(),
        piece_length: 16384,
        pieces: vec![0u8; 20],
        length: None,
        files: Some(files),
        private: None,
        source: None,
        ssl_cert: None,
        similar: vec![],
        collections: vec![],
    };
    let v1 = irontide::core::TorrentMetaV1 {
        info_hash: Id20::from([0u8; 20]),
        announce: None,
        announce_list: None,
        comment: None,
        created_by: None,
        creation_date: None,
        info,
        url_list: vec![],
        httpseeds: vec![],
        info_bytes: None,
        ssl_cert: None,
    };
    let entries = v1_file_entries(&v1);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].index, 0);
    assert_eq!(entries[0].path, "dir/file1.txt");
    assert_eq!(entries[0].size, 500);
    assert_eq!(entries[1].index, 1);
    assert_eq!(entries[1].path, "file2.txt");
    assert_eq!(entries[1].size, 1200);
}

#[test]
fn test_v1_file_entries_empty_file_list() {
    // A torrent with no files (unusual but code handles it)
    let info = irontide::core::InfoDict {
        name: "empty".into(),
        piece_length: 16384,
        pieces: vec![0u8; 20],
        length: Some(0),
        files: None,
        private: None,
        source: None,
        ssl_cert: None,
        similar: vec![],
        collections: vec![],
    };
    let v1 = irontide::core::TorrentMetaV1 {
        info_hash: Id20::from([0u8; 20]),
        announce: None,
        announce_list: None,
        comment: None,
        created_by: None,
        creation_date: None,
        info,
        url_list: vec![],
        httpseeds: vec![],
        info_bytes: None,
        ssl_cert: None,
    };
    let entries = v1_file_entries(&v1);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].index, 0);
    assert_eq!(entries[0].path, "empty");
    assert_eq!(entries[0].size, 0);
}

#[test]
fn test_preview_entries_from_meta_v1() {
    // Delegates to v1_file_entries, so a single smoke test suffices
    let info = irontide::core::InfoDict {
        name: "test.iso".into(),
        piece_length: 16384,
        pieces: vec![0u8; 20],
        length: Some(42),
        files: None,
        private: None,
        source: None,
        ssl_cert: None,
        similar: vec![],
        collections: vec![],
    };
    let v1 = irontide::core::TorrentMetaV1 {
        info_hash: Id20::from([0u8; 20]),
        announce: None,
        announce_list: None,
        comment: None,
        created_by: None,
        creation_date: None,
        info,
        url_list: vec![],
        httpseeds: vec![],
        info_bytes: None,
        ssl_cert: None,
    };
    let meta = irontide::core::TorrentMeta::V1(v1);
    let entries = preview_entries_from_meta(&meta);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].path, "test.iso");
    assert_eq!(entries[0].size, 42);
}

#[test]
fn test_preview_entries_from_meta_hybrid() {
    let info = irontide::core::InfoDict {
        name: "hybrid.iso".into(),
        piece_length: 16384,
        pieces: vec![0u8; 20],
        length: Some(100),
        files: None,
        private: None,
        source: None,
        ssl_cert: None,
        similar: vec![],
        collections: vec![],
    };
    let v1 = irontide::core::TorrentMetaV1 {
        info_hash: Id20::from([0u8; 20]),
        announce: None,
        announce_list: None,
        comment: None,
        created_by: None,
        creation_date: None,
        info,
        url_list: vec![],
        httpseeds: vec![],
        info_bytes: None,
        ssl_cert: None,
    };
    // Hybrid uses the v1 info dict for file entries
    let v2 = irontide::core::TorrentMetaV2 {
        info_hashes: InfoHashes::v2_only(Id32::from([0u8; 32])),
        info_bytes: None,
        announce: None,
        announce_list: None,
        comment: None,
        created_by: None,
        creation_date: None,
        info: InfoDictV2 {
            name: "hybrid.iso".into(),
            piece_length: 16384,
            meta_version: 2,
            file_tree: FileTreeNode::Directory(BTreeMap::new()),
            ssl_cert: None,
        },
        piece_layers: BTreeMap::new(),
        ssl_cert: None,
    };
    let meta = irontide::core::TorrentMeta::Hybrid(Box::new(v1), Box::new(v2));
    let entries = preview_entries_from_meta(&meta);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].path, "hybrid.iso");
    assert_eq!(entries[0].size, 100);
}

#[test]
fn test_preview_entries_from_meta_v2() {
    // V2 torrents return a placeholder entry because we don't parse V2 file trees
    let meta = irontide::core::TorrentMeta::V2(irontide::core::TorrentMetaV2 {
        info_hashes: InfoHashes::v2_only(Id32::from([0u8; 32])),
        info_bytes: None,
        announce: None,
        announce_list: None,
        comment: None,
        created_by: None,
        creation_date: None,
        info: InfoDictV2 {
            name: "v2-torrent".into(),
            piece_length: 16384,
            meta_version: 2,
            file_tree: FileTreeNode::Directory(BTreeMap::new()),
            ssl_cert: None,
        },
        piece_layers: BTreeMap::new(),
        ssl_cert: None,
    });
    let entries = preview_entries_from_meta(&meta);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].index, 0);
    assert_eq!(entries[0].path, "v2-torrent");
    assert_eq!(entries[0].size, 0);
}

#[test]
fn test_v1_file_entries_nested_paths() {
    let files = vec![irontide::core::FileEntry {
        length: 200,
        path: vec!["a".into(), "b".into(), "c".into(), "deep.txt".into()],
        attr: None,
        mtime: None,
        symlink_path: None,
    }];
    let info = irontide::core::InfoDict {
        name: "root".into(),
        piece_length: 16384,
        pieces: vec![0u8; 20],
        length: None,
        files: Some(files),
        private: None,
        source: None,
        ssl_cert: None,
        similar: vec![],
        collections: vec![],
    };
    let v1 = irontide::core::TorrentMetaV1 {
        info_hash: Id20::from([0u8; 20]),
        announce: None,
        announce_list: None,
        comment: None,
        created_by: None,
        creation_date: None,
        info,
        url_list: vec![],
        httpseeds: vec![],
        info_bytes: None,
        ssl_cert: None,
    };
    let entries = v1_file_entries(&v1);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].index, 0);
    assert_eq!(entries[0].path, "a/b/c/deep.txt");
    assert_eq!(entries[0].size, 200);
}
