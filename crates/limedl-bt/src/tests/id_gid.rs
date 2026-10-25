//! Internal id -> aria2 GID conversion.

use super::*;

#[test]
fn test_internal_id_to_gid_known_hash() {
    let ih = Id20::from([0u8; 20]);
    let gid = internal_id_to_gid(&ih);
    // xxh3_64 of 40 zero hex chars should be deterministic
    assert_eq!(gid.len(), 16);
    assert!(gid.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_internal_id_to_gid_different_hashes_different_gids() {
    let ih1 = Id20::from([1u8; 20]);
    let ih2 = Id20::from([2u8; 20]);
    assert_ne!(internal_id_to_gid(&ih1), internal_id_to_gid(&ih2));
}

#[test]
fn test_internal_id_to_gid_same_hash_same_gid() {
    let ih = Id20::from([42u8; 20]);
    assert_eq!(internal_id_to_gid(&ih), internal_id_to_gid(&ih));
}

#[test]
fn test_internal_id_to_gid_consistent_with_aria2_format() {
    // Verify GID is always 16 lowercase hex chars (matching aria2 format)
    let ih = Id20::from([0xABu8; 20]);
    let gid = internal_id_to_gid(&ih);
    assert_eq!(gid.len(), 16);
    assert!(gid.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(gid, gid.to_ascii_lowercase(), "GID must be lowercase");
}
