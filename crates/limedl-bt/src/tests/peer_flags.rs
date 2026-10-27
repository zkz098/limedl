//! Peer flag and client-name helpers.

use super::*;

#[test]
fn test_build_peer_flags_empty() {
    assert_eq!(build_peer_flags(None::<&str>), "");
}

#[test]
fn test_build_peer_flags_utp() {
    assert_eq!(build_peer_flags(Some("Utp")), "u");
}

#[test]
fn test_build_peer_flags_tcp() {
    assert_eq!(build_peer_flags(Some("Tcp")), "T");
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
