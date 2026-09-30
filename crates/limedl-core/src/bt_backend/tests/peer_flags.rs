//! Peer flag and client-name helpers.

use super::*;

#[test]
fn test_build_peer_flags_empty() {
    let peer = make_peer();
    assert_eq!(build_peer_flags(&peer), "");
}

#[test]
fn test_build_peer_flags_all() {
    let mut peer = make_peer();
    peer.is_encrypted = true;
    peer.uses_utp = true;
    peer.supports_fast = true;
    peer.upload_only = true;
    peer.snubbed = true;
    peer.am_choking = true;
    peer.peer_interested = true;
    assert_eq!(build_peer_flags(&peer), "EuFUScI");
}

#[test]
fn test_build_peer_flags_encrypted() {
    let mut peer = make_peer();
    peer.is_encrypted = true;
    assert_eq!(build_peer_flags(&peer), "E");
}

#[test]
fn test_build_peer_flags_utp() {
    let mut peer = make_peer();
    peer.uses_utp = true;
    assert_eq!(build_peer_flags(&peer), "u");
}

#[test]
fn test_build_peer_flags_fast() {
    let mut peer = make_peer();
    peer.supports_fast = true;
    assert_eq!(build_peer_flags(&peer), "F");
}

#[test]
fn test_build_peer_flags_upload_only() {
    let mut peer = make_peer();
    peer.upload_only = true;
    assert_eq!(build_peer_flags(&peer), "U");
}

#[test]
fn test_build_peer_flags_snubbed() {
    let mut peer = make_peer();
    peer.snubbed = true;
    assert_eq!(build_peer_flags(&peer), "S");
}

#[test]
fn test_build_peer_flags_am_choking() {
    let mut peer = make_peer();
    peer.am_choking = true;
    assert_eq!(build_peer_flags(&peer), "c");
}

#[test]
fn test_build_peer_flags_interested() {
    let mut peer = make_peer();
    peer.peer_interested = true;
    assert_eq!(build_peer_flags(&peer), "I");
}

#[test]
fn test_build_peer_flags_combination() {
    let mut peer = make_peer();
    peer.is_encrypted = true;
    peer.supports_fast = true;
    peer.am_choking = true;
    // E + F + c
    assert_eq!(build_peer_flags(&peer), "EFc");
}

#[test]
fn test_sanitize_peer_client() {
    assert_eq!(sanitize_peer_client(""), "");
    assert_eq!(sanitize_peer_client("  Gopeed dev  "), "Gopeed dev");
    assert_eq!(
        sanitize_peer_client("qBittorrent/5.0.0\0"),
        "qBittorrent/5.0.0"
    );
    assert_eq!(
        sanitize_peer_client("Transmission\u{0001}\u{0007}"),
        "Transmission"
    );
    assert_eq!(sanitize_peer_client("\0\r\n\t"), "");
}
