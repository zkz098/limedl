use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use reqwest::{Client, Proxy, redirect::Policy};

use super::ip_ranges::{CLOUDFLARE_IPV4_RANGES, network_address, parse_cidr};
use crate::download::types::{AppSettings, ProxyMode};

/// Cloudflare IPv6 CIDR ranges.
/// Source: https://www.cloudflare.com/ips-v6
const CLOUDFLARE_IPV6_RANGES: &[&str] = &[
    "2606:4700::/32",
    "2803:f800::/32",
    "2405:b500::/32",
    "2405:8100::/32",
    "2a06:98c0::/29",
    "2c0f:f248::/32",
];

/// Build a separate reqwest Client that resolves `domain` to a specific IPv4 address.
///
/// All TCP/TLS settings mirror `build_http_client()` in manager.rs (lines 2449-2470).
/// The DNS override via `resolve_to_addrs` sends `domain` as the TLS SNI hostname
/// so certificate validation works correctly against the original domain name.
/// Port 0 in the socket address means "use URL scheme default" (443 for https, 80 for http).
pub(crate) fn build_accelerated_client(
    domain: &str,
    ip: Ipv4Addr,
    settings: &AppSettings,
) -> Result<Client, Box<dyn std::error::Error + Send + Sync>> {
    let mut builder = Client::builder()
        .redirect(Policy::limited(10))
        .tcp_nodelay(true)
        .read_timeout(Duration::from_secs(15))
        .user_agent(settings.download.default_user_agent.clone());

    match settings.proxy.mode {
        ProxyMode::Disabled => {
            builder = builder.no_proxy();
        }
        ProxyMode::System => {
            // Use reqwest default (environment variables).
        }
        ProxyMode::Manual => {
            let proxy = Proxy::all(&settings.proxy.manual_url)?;
            builder = builder.proxy(proxy);
        }
    }

    // DNS override: resolve the domain to the specified IP.
    // Port 0 means "use URL scheme default" (443 for https, 80 for http).
    builder = builder.resolve_to_addrs(domain, &[SocketAddr::new(IpAddr::V4(ip), 0)]);

    let client = builder.build()?;
    Ok(client)
}

/// Check whether a single IPv4 address falls within any Cloudflare CIDR range.
///
/// This is a pure, synchronous function — no DNS involved. Useful for unit testing
/// the CIDR matching logic independently from network resolution.
pub(crate) fn ip_in_cloudflare_ranges(ip: Ipv4Addr) -> bool {
    CLOUDFLARE_IPV4_RANGES.iter().any(|cidr_str| {
        if let Some((network, prefix)) = parse_cidr(cidr_str) {
            network_address(ip, prefix) == network
        } else {
            false
        }
    })
}

/// Check whether a single IPv6 address falls within any Cloudflare IPv6 CIDR range.
fn ip_in_cloudflare_ipv6_ranges(ip: Ipv6Addr) -> bool {
    CLOUDFLARE_IPV6_RANGES.iter().any(|cidr_str| {
        let Some((net_str, prefix_str)) = cidr_str.split_once('/') else {
            return false;
        };
        let Ok(prefix) = prefix_str.parse::<u8>() else {
            return false;
        };
        if prefix > 128 {
            return false;
        }
        let Ok(net) = net_str.parse::<Ipv6Addr>() else {
            return false;
        };
        let raw = ip.to_bits();
        let mask = if prefix == 0 {
            0u128
        } else {
            !0u128 << (128 - prefix)
        };
        (net.to_bits() & mask) == (raw & mask)
    })
}

/// Check whether a URL targets a Cloudflare domain.
///
/// Parses the URL hostname, resolves it via DNS (3s timeout), and checks if any
/// resolved IPv4 address falls within a known Cloudflare CIDR range.
///
/// Returns `false` gracefully for any failure: URL parse errors, DNS resolution
/// failures, timeouts, or empty resolve results.
pub(crate) async fn is_cloudflare_domain(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        tracing::debug!("is_cloudflare_domain: failed to parse URL: {url}");
        return false;
    };

    let Some(hostname) = parsed.host_str() else {
        tracing::debug!("is_cloudflare_domain: no host in URL: {url}");
        return false;
    };

    // DNS resolution with 3s timeout.
    // Port 0 avoids actual connection — only resolves the address.
    let addrs = match tokio::time::timeout(
        Duration::from_secs(3),
        tokio::net::lookup_host(format!("{hostname}:0")),
    )
    .await
    {
        Ok(Ok(addrs)) => addrs,
        _ => {
            tracing::debug!("is_cloudflare_domain: DNS resolution failed for {hostname}");
            return false;
        }
    };

    // Check if any resolved address is in Cloudflare CIDR ranges (both IPv4 and IPv6).
    addrs.into_iter().any(|addr| match addr.ip() {
        IpAddr::V4(v4) => ip_in_cloudflare_ranges(v4),
        IpAddr::V6(v6) => ip_in_cloudflare_ipv6_ranges(v6),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_accelerated_client() {
        let settings = AppSettings::default();
        let localhost = Ipv4Addr::new(127, 0, 0, 1);
        let result = build_accelerated_client("example.com", localhost, &settings);
        assert!(result.is_ok(), "should build client without panicking");
    }

    // ── ip_in_cloudflare_ranges unit tests ──

    #[test]
    fn test_ip_in_cloudflare_ranges_known_cf_ip() {
        // 104.16.0.1 is within 104.16.0.0/13 (network = 104.16.0.0, prefix = 13)
        assert!(ip_in_cloudflare_ranges(Ipv4Addr::new(104, 16, 0, 1)));
    }

    #[test]
    fn test_ip_in_cloudflare_ranges_google_dns() {
        // 8.8.8.8 is Google DNS, not Cloudflare
        assert!(!ip_in_cloudflare_ranges(Ipv4Addr::new(8, 8, 8, 8)));
    }

    #[test]
    fn test_ip_in_cloudflare_ranges_cloudflare_dns() {
        // 1.1.1.1 is Cloudflare's public DNS resolver, NOT in the reverse-proxy CIDR list
        assert!(!ip_in_cloudflare_ranges(Ipv4Addr::new(1, 1, 1, 1)));
    }

    #[test]
    fn test_ip_in_cloudflare_ranges_unspecified() {
        assert!(!ip_in_cloudflare_ranges(Ipv4Addr::UNSPECIFIED));
    }

    #[test]
    fn test_ip_in_cloudflare_ranges_broadcast() {
        assert!(!ip_in_cloudflare_ranges(Ipv4Addr::BROADCAST));
    }

    #[test]
    fn test_ip_in_cloudflare_ranges_network_boundary() {
        // 104.16.0.0 is the network address of 104.16.0.0/13 — should match
        assert!(ip_in_cloudflare_ranges(Ipv4Addr::new(104, 16, 0, 0)));
        // 104.23.255.255 is the broadcast of 104.16.0.0/13 — should match
        assert!(ip_in_cloudflare_ranges(Ipv4Addr::new(104, 23, 255, 255)));
        // TEST-NET-3 (203.0.113.0/24) — documentation-only, not in any CF range
        assert!(!ip_in_cloudflare_ranges(Ipv4Addr::new(203, 0, 113, 1)));
    }

    // ── ip_in_cloudflare_ipv6_ranges unit tests ──

    #[test]
    fn test_ip_in_cloudflare_ipv6_ranges_known_cf_ip() {
        // 2606:4700::1 is within 2606:4700::/32
        assert!(ip_in_cloudflare_ipv6_ranges(Ipv6Addr::new(
            0x2606, 0x4700, 0, 0, 0, 0, 0, 1
        )));
    }

    #[test]
    fn test_ip_in_cloudflare_ipv6_ranges_google_dns() {
        // 2001:4860:4860::8888 is Google DNS, not Cloudflare
        assert!(!ip_in_cloudflare_ipv6_ranges(Ipv6Addr::new(
            0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888
        )));
    }

    #[test]
    fn test_ip_in_cloudflare_ipv6_ranges_network_boundary() {
        // 2606:4700:: is the network address of 2606:4700::/32 — should match
        assert!(ip_in_cloudflare_ipv6_ranges(Ipv6Addr::new(
            0x2606, 0x4700, 0, 0, 0, 0, 0, 0
        )));
        // 2606:4700:ffff:ffff:ffff:ffff:ffff:ffff is the broadcast — should match
        assert!(ip_in_cloudflare_ipv6_ranges(Ipv6Addr::new(
            0x2606, 0x4700, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff
        )));
        // 2606:4800:: is NOT in 2606:4700::/32 — should not match
        assert!(!ip_in_cloudflare_ipv6_ranges(Ipv6Addr::new(
            0x2606, 0x4800, 0, 0, 0, 0, 0, 0
        )));
    }

    // ── is_cloudflare_domain async tests ──

    #[tokio::test]
    async fn test_is_cloudflare_domain_invalid_url() {
        assert!(!is_cloudflare_domain("").await);
        assert!(!is_cloudflare_domain("not-a-url!!!").await);
        assert!(!is_cloudflare_domain("ht!tp://bad.url").await);
    }

    #[tokio::test]
    async fn test_is_cloudflare_domain_non_cloudflare() {
        // httpbin.org is hosted on AWS, not Cloudflare
        assert!(!is_cloudflare_domain("https://httpbin.org/").await);
    }
}
