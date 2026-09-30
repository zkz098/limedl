//! Live irontide session lifecycle.

use super::*;

#[tokio::test]
async fn test_session_create_and_shutdown() {
    let (_tmp, session) = make_test_session().await;

    // Verify empty session
    let stats = session.session_stats().await.unwrap();
    assert_eq!(stats.active_torrents, 0);

    let list = session.list_torrents().await.unwrap();
    assert!(list.is_empty());

    session.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_session_magnet_add_and_remove() {
    let (_tmp, session) = make_test_session().await;

    // Add a magnet link
    let magnet = irontide::core::Magnet::parse(
        "magnet:?xt=urn:btih:aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d&dn=test",
    )
    .unwrap();
    let info_hash = irontide::AddTorrentParams::from_magnet(magnet)
        .add_to(&session)
        .await
        .unwrap();

    assert_eq!(
        info_hash.to_hex(),
        "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d"
    );

    // Verify it's listed
    let list = session.list_torrents().await.unwrap();
    assert_eq!(list.len(), 1);
    assert!(list.contains(&info_hash));

    // Remove it
    session.remove_torrent(info_hash).await.unwrap();

    let list = session.list_torrents().await.unwrap();
    assert!(list.is_empty());

    session.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_session_magnet_add_get_stats() {
    let (_tmp, session) = make_test_session().await;

    // Add a magnet link
    let magnet = irontide::core::Magnet::parse(
        "magnet:?xt=urn:btih:da39a3ee5e6b4b0d3255bfef95601890afd80709&dn=empty",
    )
    .unwrap();
    let info_hash = irontide::AddTorrentParams::from_magnet(magnet)
        .add_to(&session)
        .await
        .unwrap();

    // Stats should be retrievable even for magnet-only torrents
    let stats = session.torrent_stats(info_hash).await.unwrap();
    // Name is empty for magnet links without resolved metadata
    assert!(!stats.has_metadata, "magnet links have no metadata yet");
    assert_eq!(
        stats.state,
        irontide::session::TorrentState::FetchingMetadata
    );

    session.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_session_pause_resume_magnet() {
    let (_tmp, session) = make_test_session().await;

    // Add a magnet link
    let magnet = irontide::core::Magnet::parse(
        "magnet:?xt=urn:btih:4a8eeb4c2f4f3ae1e2a8a3d4b5c6d7e8f9a0b1c2&dn=pause-test",
    )
    .unwrap();
    let info_hash = irontide::AddTorrentParams::from_magnet(magnet)
        .add_to(&session)
        .await
        .unwrap();

    // Pause
    session.pause_torrent(info_hash).await.unwrap();
    let stats = session.torrent_stats(info_hash).await.unwrap();
    assert_eq!(stats.state, irontide::session::TorrentState::Paused);

    // Resume
    session.resume_torrent(info_hash).await.unwrap();
    let stats = session.torrent_stats(info_hash).await.unwrap();
    assert_eq!(
        stats.state,
        irontide::session::TorrentState::FetchingMetadata
    );

    session.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_session_multiple_torrents_listed() {
    let (_tmp, session) = make_test_session().await;

    let magnet1 = irontide::core::Magnet::parse(
        "magnet:?xt=urn:btih:aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d&dn=one",
    )
    .unwrap();
    let info_hash1 = irontide::AddTorrentParams::from_magnet(magnet1)
        .add_to(&session)
        .await
        .unwrap();

    let magnet2 = irontide::core::Magnet::parse(
        "magnet:?xt=urn:btih:da39a3ee5e6b4b0d3255bfef95601890afd80709&dn=two",
    )
    .unwrap();
    let info_hash2 = irontide::AddTorrentParams::from_magnet(magnet2)
        .add_to(&session)
        .await
        .unwrap();

    let list = session.list_torrents().await.unwrap();
    assert_eq!(list.len(), 2);
    assert!(list.contains(&info_hash1));
    assert!(list.contains(&info_hash2));

    // Get individual stats (names are empty before metadata resolution)
    let stats1 = session.torrent_stats(info_hash1).await.unwrap();
    assert!(!stats1.has_metadata);
    let stats2 = session.torrent_stats(info_hash2).await.unwrap();
    assert!(!stats2.has_metadata);
    // The two torrents are different
    assert_ne!(info_hash1, info_hash2);

    session.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_session_session_stats_accessible() {
    let (_tmp, session) = make_test_session().await;

    // Add a magnet to have some session activity
    let magnet = irontide::core::Magnet::parse(
        "magnet:?xt=urn:btih:aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d&dn=stats-test",
    )
    .unwrap();
    let _info_hash = irontide::AddTorrentParams::from_magnet(magnet)
        .add_to(&session)
        .await
        .unwrap();

    let session_stats = session.session_stats().await.unwrap();
    assert_eq!(session_stats.active_torrents, 1);

    session.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_session_remove_with_files_and_readd() {
    let (_tmp, session) = make_test_session().await;

    let magnet = irontide::core::Magnet::parse(
        "magnet:?xt=urn:btih:aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d&dn=remove-me",
    )
    .unwrap();
    let info_hash = irontide::AddTorrentParams::from_magnet(magnet)
        .add_to(&session)
        .await
        .unwrap();

    // Remove with files
    session.remove_torrent_with_files(info_hash).await.unwrap();
    let list = session.list_torrents().await.unwrap();
    assert!(list.is_empty());

    // Re-add the same magnet
    let magnet2 = irontide::core::Magnet::parse(
        "magnet:?xt=urn:btih:aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d&dn=readded",
    )
    .unwrap();
    let info_hash2 = irontide::AddTorrentParams::from_magnet(magnet2)
        .add_to(&session)
        .await
        .unwrap();
    assert_eq!(info_hash, info_hash2, "same info hash after re-add");

    let list = session.list_torrents().await.unwrap();
    assert_eq!(list.len(), 1);

    session.shutdown().await.unwrap();
}
