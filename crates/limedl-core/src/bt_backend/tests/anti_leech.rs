//! Anti-leech heuristics.

use super::*;

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
