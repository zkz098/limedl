#![no_main]

use libfuzzer_sys::fuzz_target;
use limedl_bt::librqbit::{Magnet, torrent_from_bytes};
use limedl_bt::queries::sanitize_peer_client;

fuzz_target!(|data: &[u8]| {
    let _ = torrent_from_bytes(data);

    if let Ok(text) = std::str::from_utf8(data) {
        let sanitized = sanitize_peer_client(text);
        assert!(sanitized.len() <= text.len());

        let _ = Magnet::parse(text);
        let prefixed = format!("magnet:?xt=urn:btih:{text}");
        let _ = Magnet::parse(&prefixed);
    }
});
