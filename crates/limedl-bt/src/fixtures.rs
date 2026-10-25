//! Deterministic `.torrent` fixtures.
//!
//! They live outside `tests/` because two consumers need the same bytes: this
//! crate's own backend tests, and `limedl-rpc`'s cross-stack `bootstrap()` e2e
//! tests (which assert the info-hash the Aria2 layer reports). Sharing them here
//! is what keeps `irontide` named in exactly one `Cargo.toml` — the alternative
//! was a dev-dependency from the engine on this crate, which would have dragged
//! the BT engine into `limedl-core --no-default-features --all-targets`.

use std::path::PathBuf;

/// Raw bencode for a minimal multi-file v1 torrent: `a.txt` (10 bytes) and
/// `b.txt` (20 bytes) behind a single 256-byte piece.
///
/// The 20-byte piece hash is all zeros on purpose: parsing only needs the
/// shape. Nothing in these tests verifies payload hashes — the files are never
/// downloaded — and this mirrors the minimal fixture irontide's own tests use.
pub fn multi_file_torrent_bytes() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"d4:infod5:filesl");
    bytes.extend_from_slice(b"d6:lengthi10e4:pathl5:a.txtee");
    bytes.extend_from_slice(b"d6:lengthi20e4:pathl5:b.txtee");
    bytes.extend_from_slice(b"e4:name4:test12:piece lengthi256e6:pieces20:");
    bytes.extend_from_slice(&[0u8; 20]);
    bytes.extend_from_slice(b"ee");
    bytes
}

/// Write [`multi_file_torrent_bytes`] into `dir` and return the file path.
pub fn write_torrent_fixture(dir: &std::path::Path) -> PathBuf {
    let path = dir.join("fixture.torrent");
    std::fs::write(&path, multi_file_torrent_bytes()).expect("write torrent fixture");
    path
}

/// Raw bencode for a minimal single-file v1 torrent: `single.bin` (30 bytes)
/// behind a single 256-byte piece.
///
/// `length`/`name` replace the `files` list on purpose: this is the shape
/// that has no per-file entries to enumerate, so file-list code must
/// synthesize the one entry itself. The piece hash is all zeros for the same
/// reason as [`multi_file_torrent_bytes`].
pub fn single_file_torrent_bytes() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(
        b"d4:infod6:lengthi30e4:name10:single.bin12:piece lengthi256e6:pieces20:",
    );
    bytes.extend_from_slice(&[0u8; 20]);
    bytes.extend_from_slice(b"ee");
    bytes
}

/// Write [`single_file_torrent_bytes`] into `dir` and return the file path.
pub fn write_single_file_torrent_fixture(dir: &std::path::Path) -> PathBuf {
    let path = dir.join("single.torrent");
    std::fs::write(&path, single_file_torrent_bytes()).expect("write single-file torrent fixture");
    path
}
