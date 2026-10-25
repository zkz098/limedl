//! Listen-address helpers for the Aria2 RPC server.
//!
//! The server binds `listen_address:port`. Two details are easy to get wrong
//! and are centralized here:
//!
//! - an IPv6 literal needs brackets (`[::1]:6800`), otherwise `::1:6800` parses
//!   as a malformed socket address;
//! - whether an address is reachable only from this host decides whether the
//!   endpoint may run without authentication (`serve` refuses otherwise), and a
//!   hostname is not an `IpAddr`, so `localhost` needs its own branch.

use std::net::IpAddr;

/// True when `address` only reaches the local host and therefore does not need
/// authentication to be safe.
///
/// Accepts the loopback literals plus the `localhost` hostname. Everything that
/// does not parse as an IP (including a typo like `0.0.0.0 `) is treated as
/// non-loopback: unknown hosts must fail closed, not open.
pub fn is_loopback_bind_address(address: &str) -> bool {
    let host = strip_brackets(address.trim());
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    host.parse::<IpAddr>()
        .map(|ip| ip.is_loopback())
        .unwrap_or(false)
}

/// Render `host` and `port` as a socket address that `TcpListener::bind`
/// accepts, bracketing an IPv6 literal.
pub fn format_bind_addr(host: &str, port: u16) -> String {
    let trimmed = host.trim();
    match strip_brackets(trimmed).parse::<IpAddr>() {
        Ok(IpAddr::V6(_)) => format!("[{}]:{port}", strip_brackets(trimmed)),
        _ => format!("{trimmed}:{port}"),
    }
}

fn strip_brackets(host: &str) -> &str {
    host.trim_start_matches('[').trim_end_matches(']')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_detection_matches_the_bindable_loopback_spellings() {
        for loopback in ["127.0.0.1", "::1", "localhost", "LOCALHOST", "[::1]"] {
            assert!(
                is_loopback_bind_address(loopback),
                "{loopback} must be recognized as loopback"
            );
        }
    }

    #[test]
    fn loopback_detection_fails_closed_for_anything_else() {
        // Every non-loopback target — including a hostname we cannot resolve —
        // must be treated as public so the auth gate can reject it.
        for public in [
            "0.0.0.0",
            "::",
            "192.168.1.10",
            "10.0.0.1",
            "example.lan",
            "  ",
            "127.0.0.1:6800",
        ] {
            assert!(
                !is_loopback_bind_address(public),
                "{public:?} must not count as loopback"
            );
        }
    }

    #[test]
    fn format_bind_addr_brackets_ipv6_only() {
        assert_eq!(format_bind_addr("127.0.0.1", 6800), "127.0.0.1:6800");
        assert_eq!(format_bind_addr("0.0.0.0", 6800), "0.0.0.0:6800");
        assert_eq!(format_bind_addr("localhost", 6800), "localhost:6800");
        // An unbracketed IPv6 literal would parse as `::1:6800` and fail to bind.
        assert_eq!(format_bind_addr("::1", 6800), "[::1]:6800");
        assert_eq!(format_bind_addr("::", 6800), "[::]:6800");
        assert_eq!(format_bind_addr("[::1]", 6800), "[::1]:6800");
        // Surrounding whitespace from a hand-edited settings file is ignored.
        assert_eq!(format_bind_addr("  0.0.0.0  ", 6800), "0.0.0.0:6800");
    }
}
