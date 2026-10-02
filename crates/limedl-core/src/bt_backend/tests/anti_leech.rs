//! Anti-leech heuristics and the background enforcement loop.

use super::*;

use crate::types::BtAntiLeechAction;

#[test]
fn test_anti_leech_upload_only_is_not_leecher() {
    let mut peer = make_leech_candidate(true, false, 0, 1000, 0.5);
    peer.upload_only = true;
    // Even a peer that chokes us and gives nothing back is fine if it declared
    // upload-only (it is a seeder, BEP 21).
    assert!(!peer_is_leecher(
        &peer,
        Some(Duration::from_secs(1000)),
        GRACE,
        RATIO
    ));
}

#[test]
fn test_anti_leech_we_are_choking_ignored() {
    // We are choking the peer (`am_choking`), so we are not sending it data —
    // it cannot be a leecher from our perspective.
    let peer = make_leech_candidate(false, true, 0, 0, 0.5);
    assert!(!peer_is_leecher(
        &peer,
        Some(Duration::from_secs(1000)),
        GRACE,
        RATIO
    ));
}

#[test]
fn test_anti_leech_within_grace_period_ignored() {
    // Unchoked for less than the grace period → not flagged (warm-up).
    let peer = make_leech_candidate(true, false, 0, 1000, 0.5);
    assert!(!peer_is_leecher(
        &peer,
        Some(Duration::from_secs(GRACE - 1)),
        GRACE,
        RATIO
    ));
}

#[test]
fn test_anti_leech_completed_peer_is_seeder() {
    // Fully downloaded peer (progress >= 1.0) is a seeder sharing back.
    let peer = make_leech_candidate(true, false, 0, 1000, 1.0);
    assert!(!peer_is_leecher(
        &peer,
        Some(Duration::from_secs(1000)),
        GRACE,
        RATIO
    ));
}

#[test]
fn test_anti_leech_choking_us_without_return_is_leecher() {
    // Classic leecher: chokes us (peer_choking) while we unchoke it for a long
    // time and receive nothing back.
    let peer = make_leech_candidate(true, false, 0, 1000, 0.5);
    assert!(peer_is_leecher(
        &peer,
        Some(Duration::from_secs(1000)),
        GRACE,
        RATIO
    ));
}

#[test]
fn test_anti_leech_low_giveback_share_is_leecher() {
    // Not choking us, but gives back far less than it takes → leecher by ratio.
    // download_rate / upload_rate = 10 / 500 = 0.02 < 0.1.
    let peer = make_leech_candidate(false, false, 10, 500, 0.5);
    assert!(peer_is_leecher(
        &peer,
        Some(Duration::from_secs(1000)),
        GRACE,
        RATIO
    ));
}

#[test]
fn test_anti_leech_good_giveback_share_not_leecher() {
    // download_rate / upload_rate = 40 / 100 = 0.4 >= 0.1 → fine.
    let peer = make_leech_candidate(false, false, 40, 100, 0.5);
    assert!(!peer_is_leecher(
        &peer,
        Some(Duration::from_secs(1000)),
        GRACE,
        RATIO
    ));
}

#[test]
fn test_anti_leech_ratio_check_disabled() {
    // With ratio = 0 the rate-ratio check is disabled; only the choke-based
    // leecher signal applies. This peer is not choking us → not a leecher.
    let peer = make_leech_candidate(false, false, 10, 500, 0.5);
    assert!(!peer_is_leecher(
        &peer,
        Some(Duration::from_secs(1000)),
        GRACE,
        0.0
    ));
}

// ── background loop ────────────────────────────────────────────────────

/// Poll `condition` until it holds, or panic after `timeout`.
async fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + timeout;
    while !condition() {
        assert!(
            std::time::Instant::now() < deadline,
            "condition not met within {timeout:?}"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn anti_leech_disabled_loop_cleans_up_bans_and_slot_caps() {
    let (_tmp, backend) = make_backend().await;
    backend.bt_settings.lock().anti_leech_enabled = false;

    let banned_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 7));
    let capped_hash = Id20::from([7u8; 20]);
    backend
        .banned_leechers
        .insert(banned_ip, crate::now_ms() + 60_000);
    backend.anti_leech_slot_state.insert(capped_hash, 4);

    let backend = Arc::new(backend);
    Arc::clone(&backend).spawn_anti_leech_loop();

    // The first interval tick fires immediately: switching the feature off must
    // revoke every ban and slot cap this module introduced.
    wait_until(Duration::from_secs(2), || {
        backend.banned_leechers.is_empty() && backend.anti_leech_slot_state.is_empty()
    })
    .await;

    backend.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn anti_leech_sweep_leaves_slot_caps_alone_without_peers() {
    let (tmp, backend) = make_backend().await;
    let path = write_torrent_fixture(tmp.path());
    let info_hash = backend
        .start(StartDownloadRequest {
            url: path.to_string_lossy().into_owned(),
            ..Default::default()
        })
        .await
        .expect("start fixture");
    {
        let mut settings = backend.bt_settings.lock();
        settings.anti_leech_enabled = true;
        settings.anti_leech_action = BtAntiLeechAction::LimitSlots;
        settings.anti_leech_max_upload_slots = 2;
    }
    // Pretend a previous sweep capped this torrent at 2 slots (originally 5).
    backend.anti_leech_slot_state.insert(info_hash, 5);
    let mut rx = backend.event_bus.subscribe();

    let backend = Arc::new(backend);
    Arc::clone(&backend).spawn_anti_leech_loop();

    // The first interval tick fires immediately. With no connected peers the
    // sweep returns before deciding anything: it cannot tell whether the
    // leechers cleared until a peer is visible again, so the cap stays put.
    tokio::time::timeout(Duration::from_millis(300), rx.recv())
        .await
        .expect_err("a peerless sweep must not emit a restore warning");
    assert_eq!(
        backend
            .anti_leech_slot_state
            .get(&info_hash)
            .map(|entry| *entry.value()),
        Some(5),
        "the recorded slot cap survives a sweep with no peers"
    );

    backend.shutdown().await;
}

#[tokio::test]
async fn anti_leech_ban_sweep_unbans_expired_entries_only() {
    let (_tmp, backend) = make_backend().await;
    {
        let mut settings = backend.bt_settings.lock();
        settings.anti_leech_enabled = true;
        settings.anti_leech_action = BtAntiLeechAction::Ban;
    }
    let expired = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 8));
    let active = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9));
    backend.banned_leechers.insert(expired, 1); // long past
    backend
        .banned_leechers
        .insert(active, crate::now_ms() + 3_600_000);

    let backend = Arc::new(backend);
    Arc::clone(&backend).spawn_anti_leech_loop();

    wait_until(Duration::from_secs(2), || {
        !backend.banned_leechers.contains_key(&expired)
    })
    .await;
    assert!(
        backend.banned_leechers.contains_key(&active),
        "an unexpired ban must survive the sweep"
    );

    backend.shutdown().await;
}

#[tokio::test]
async fn anti_leech_spawn_replaces_the_previous_loop() {
    let (_tmp, backend) = make_backend().await;
    let backend = Arc::new(backend);

    Arc::clone(&backend).spawn_anti_leech_loop();
    let first = backend.anti_leech_task.lock().as_ref().map(|h| h.id());
    assert!(first.is_some(), "spawn stores a join handle");

    Arc::clone(&backend).spawn_anti_leech_loop();
    let second = backend.anti_leech_task.lock().as_ref().map(|h| h.id());
    assert!(second.is_some(), "the replacement loop is stored");
    assert_ne!(first, second, "spawn must abort and replace the previous loop");

    backend.shutdown().await;
}
