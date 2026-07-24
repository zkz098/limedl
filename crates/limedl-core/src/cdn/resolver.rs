use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use reqwest::Client;

use super::ip_ranges::{self, CdnIpCache, FALLBACK_CACHE};
use crate::http_client_factory::configure_client_builder;
use crate::types::AppSettings;

/// TTL for cached `is_cloudflare_domain()` DNS results.
const DNS_CACHE_TTL: Duration = Duration::from_secs(300);

/// Cached DNS resolution results for `is_cloudflare_domain()`.
/// Maps hostname → (is_cloudflare, cached_at).
static DNS_CACHE: LazyLock<Mutex<HashMap<String, (bool, Instant)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Build a separate reqwest Client that resolves `domain` to a specific IP address.
///
/// All TCP/TLS settings mirror `build_http_client()` in manager.rs.
/// The DNS override via `resolve_to_addrs` sends `domain` as the TLS SNI hostname
/// so certificate validation works correctly against the original domain name.
/// Port 0 in the socket address means "use URL scheme default" (443 for https, 80 for http).
/// Accepts both IPv4 and IPv6 addresses.
pub fn build_accelerated_client(
    domain: &str,
    ip: IpAddr,
    settings: &AppSettings,
) -> Result<Client, Box<dyn std::error::Error + Send + Sync>> {
    let builder = Client::builder();
    let mut builder = configure_client_builder(builder, settings)
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    // DNS override: resolve the domain to the specified IP.
    // Port 0 means "use URL scheme default" (443 for https, 80 for http).
    builder = builder.resolve_to_addrs(domain, &[SocketAddr::new(ip, 0)]);

    let client = builder.build()?;
    Ok(client)
}

/// Check whether a single IPv4 address falls within any of the given Cloudflare CIDR ranges.
///
/// This is a pure, synchronous function — no DNS involved.
pub fn ip_in_cloudflare_ranges(ip: Ipv4Addr, cidrs: &[String]) -> bool {
    cidrs.iter().any(|cidr_str| {
        if let Some((network, prefix)) = ip_ranges::parse_cidr(cidr_str) {
            ip_ranges::network_address(ip, prefix) == network
        } else {
            false
        }
    })
}

/// Check whether a single IPv6 address falls within any of the given Cloudflare IPv6 CIDR ranges.
pub fn ip_in_cloudflare_ipv6_ranges(ip: Ipv6Addr, cidrs: &[String]) -> bool {
    cidrs.iter().any(|cidr_str| {
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
/// resolved IP address falls within known Cloudflare CIDR ranges.
///
/// Uses `cache` for Cloudflare CIDR lists when available, falling back to the
/// static [`FALLBACK_CACHE`] when no dynamic cache has been populated yet.
///
/// Returns `false` gracefully for any failure: URL parse errors, DNS resolution
/// failures, timeouts, or empty resolve results.
pub async fn is_cloudflare_domain(url: &str, cache: Option<&CdnIpCache>) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        tracing::debug!("is_cloudflare_domain: failed to parse URL: {url}");
        return false;
    };

    let Some(hostname) = parsed.host_str() else {
        tracing::debug!("is_cloudflare_domain: no host in URL: {url}");
        return false;
    };

    // Check cache first.
    {
        let dns_cache = DNS_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((result, cached_at)) = dns_cache.get(hostname)
            && cached_at.elapsed() < DNS_CACHE_TTL
        {
            return *result;
        }
    }

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
            // DNS failures are transient — do NOT cache so retries are possible.
            return false;
        }
    };

    // Use the provided cache or fall back to static ranges.
    let ip_cache = cache.unwrap_or(&FALLBACK_CACHE);

    // Check if any resolved address is in Cloudflare CIDR ranges (both IPv4 and IPv6).
    let is_cf = addrs.into_iter().any(|addr| match addr.ip() {
        IpAddr::V4(v4) => ip_in_cloudflare_ranges(v4, &ip_cache.ipv4_cidrs),
        IpAddr::V6(v6) => ip_in_cloudflare_ipv6_ranges(v6, &ip_cache.ipv6_cidrs),
    });

    // Cache the result (both true and false from a successful DNS lookup).
    {
        let mut dns_cache = DNS_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        dns_cache.insert(hostname.to_string(), (is_cf, Instant::now()));
    }

    is_cf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_accelerated_client() {
        let settings = AppSettings::default();
        let localhost = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let result = build_accelerated_client("example.com", localhost, &settings);
        assert!(result.is_ok(), "should build client without panicking");
    }

    #[test]
    fn test_build_accelerated_client_ipv6() {
        let settings = AppSettings::default();
        let localhost = IpAddr::V6(Ipv6Addr::LOCALHOST);
        let result = build_accelerated_client("example.com", localhost, &settings);
        assert!(result.is_ok(), "should build IPv6 client without panicking");
    }

    // ── ip_in_cloudflare_ranges unit tests ──

    #[test]
    fn test_ip_in_cloudflare_ranges_known_cf_ip() {
        let cidrs = &FALLBACK_CACHE.ipv4_cidrs;
        // 104.16.0.1 is within 104.16.0.0/13 (network = 104.16.0.0, prefix = 13)
        assert!(ip_in_cloudflare_ranges(Ipv4Addr::new(104, 16, 0, 1), cidrs));
    }

    #[test]
    fn test_ip_in_cloudflare_ranges_google_dns() {
        let cidrs = &FALLBACK_CACHE.ipv4_cidrs;
        // 8.8.8.8 is Google DNS, not Cloudflare
        assert!(!ip_in_cloudflare_ranges(Ipv4Addr::new(8, 8, 8, 8), cidrs));
    }

    #[test]
    fn test_ip_in_cloudflare_ranges_cloudflare_dns() {
        let cidrs = &FALLBACK_CACHE.ipv4_cidrs;
        // 1.1.1.1 is Cloudflare's public DNS resolver, NOT in the reverse-proxy CIDR list
        assert!(!ip_in_cloudflare_ranges(Ipv4Addr::new(1, 1, 1, 1), cidrs));
    }

    #[test]
    fn test_ip_in_cloudflare_ranges_unspecified() {
        let cidrs = &FALLBACK_CACHE.ipv4_cidrs;
        assert!(!ip_in_cloudflare_ranges(Ipv4Addr::UNSPECIFIED, cidrs));
    }

    #[test]
    fn test_ip_in_cloudflare_ranges_broadcast() {
        let cidrs = &FALLBACK_CACHE.ipv4_cidrs;
        assert!(!ip_in_cloudflare_ranges(Ipv4Addr::BROADCAST, cidrs));
    }

    #[test]
    fn test_ip_in_cloudflare_ranges_network_boundary() {
        let cidrs = &FALLBACK_CACHE.ipv4_cidrs;
        // 104.16.0.0 is the network address of 104.16.0.0/13 — should match
        assert!(ip_in_cloudflare_ranges(Ipv4Addr::new(104, 16, 0, 0), cidrs));
        // 104.23.255.255 is the broadcast of 104.16.0.0/13 — should match
        assert!(ip_in_cloudflare_ranges(Ipv4Addr::new(104, 23, 255, 255), cidrs));
        // TEST-NET-3 (203.0.113.0/24) — documentation-only, not in any CF range
        assert!(!ip_in_cloudflare_ranges(Ipv4Addr::new(203, 0, 113, 1), cidrs));
    }

    // ── ip_in_cloudflare_ipv6_ranges unit tests ──

    #[test]
    fn test_ip_in_cloudflare_ipv6_ranges_known_cf_ip() {
        let cidrs = &FALLBACK_CACHE.ipv6_cidrs;
        // 2606:4700::1 is within 2606:4700::/32
        assert!(ip_in_cloudflare_ipv6_ranges(Ipv6Addr::new(
            0x2606, 0x4700, 0, 0, 0, 0, 0, 1
        ), cidrs));
    }

    #[test]
    fn test_ip_in_cloudflare_ipv6_ranges_google_dns() {
        let cidrs = &FALLBACK_CACHE.ipv6_cidrs;
        // 2001:4860:4860::8888 is Google DNS, not Cloudflare
        assert!(!ip_in_cloudflare_ipv6_ranges(Ipv6Addr::new(
            0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888
        ), cidrs));
    }

    #[test]
    fn test_ip_in_cloudflare_ipv6_ranges_network_boundary() {
        let cidrs = &FALLBACK_CACHE.ipv6_cidrs;
        // 2606:4700:: is the network address of 2606:4700::/32 — should match
        assert!(ip_in_cloudflare_ipv6_ranges(Ipv6Addr::new(
            0x2606, 0x4700, 0, 0, 0, 0, 0, 0
        ), cidrs));
        // 2606:4700:ffff:ffff:ffff:ffff:ffff:ffff is the broadcast — should match
        assert!(ip_in_cloudflare_ipv6_ranges(Ipv6Addr::new(
            0x2606, 0x4700, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff
        ), cidrs));
        // 2606:4800:: is NOT in 2606:4700::/32 — should not match
        assert!(!ip_in_cloudflare_ipv6_ranges(Ipv6Addr::new(
            0x2606, 0x4800, 0, 0, 0, 0, 0, 0
        ), cidrs));
    }

    // ── is_cloudflare_domain async tests ──

    #[tokio::test]
    async fn test_is_cloudflare_domain_invalid_url() {
        let cache = &*FALLBACK_CACHE;
        assert!(!is_cloudflare_domain("", Some(cache)).await);
        assert!(!is_cloudflare_domain("not-a-url!!!", Some(cache)).await);
        assert!(!is_cloudflare_domain("ht!tp://bad.url", Some(cache)).await);
    }

    #[tokio::test]
    async fn test_is_cloudflare_domain_non_cloudflare() {
        let cache = &*FALLBACK_CACHE;
        // httpbin.org is hosted on AWS, not Cloudflare
        assert!(!is_cloudflare_domain("https://httpbin.org/", Some(cache)).await);
    }

    #[tokio::test]
    async fn test_is_cloudflare_domain_fallback_cache_works() {
        // Passing None should use the static FALLBACK_CACHE
        assert!(!is_cloudflare_domain("https://httpbin.org/", None).await);
    }
}
