#![allow(dead_code)]

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

use serde::Deserialize;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

/// Cache TTL: 24 hours before considering cached IP ranges stale.
pub const CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// Static fallback list of Cloudflare IPv4 CIDR ranges.
///
/// Source: <https://www.cloudflare.com/ips-v4> — verified June 2026.
/// These are used when live HTTP fetching of the current ranges fails.
pub const CLOUDFLARE_IPV4_RANGES: &[&str] = &[
    "173.245.48.0/20",
    "103.21.244.0/22",
    "103.22.200.0/22",
    "103.31.4.0/22",
    "141.101.64.0/18",
    "108.162.192.0/18",
    "190.93.240.0/20",
    "188.114.96.0/20",
    "197.234.240.0/22",
    "198.41.128.0/17",
    "162.158.0.0/15",
    "104.16.0.0/13",
    "104.24.0.0/14",
    "172.64.0.0/13",
    "131.0.72.0/22",
];

/// Static fallback list of Cloudflare IPv6 CIDR ranges.
///
/// Source: <https://www.cloudflare.com/ips-v6> — verified June 2026.
/// These are used when live HTTP fetching of the current ranges fails.
pub const CLOUDFLARE_IPV6_RANGES: &[&str] = &[
    "2606:4700::/32",
    "2803:f800::/32",
    "2405:b500::/32",
    "2405:8100::/32",
    "2a06:98c0::/29",
    "2c0f:f248::/32",
];

/// Pre-built static fallback cache so `is_cloudflare_domain()` can quickly
/// check against known CIDRs without re-parsing every call.
pub static FALLBACK_CACHE: LazyLock<CdnIpCache> = LazyLock::new(CdnIpCache::from_fallback);

// ── IPv4 CIDR helpers ──────────────────────────────────────────

/// Parse a single IPv4 address string into an [`Ipv4Addr`].
///
/// Returns `None` if the string is malformed or any octet is out of range.
fn parse_ipv4(s: &str) -> Option<Ipv4Addr> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 4 {
        return None;
    }
    let octets: [u8; 4] = [
        parts[0].parse().ok()?,
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
        parts[3].parse().ok()?,
    ];
    Some(Ipv4Addr::new(octets[0], octets[1], octets[2], octets[3]))
}

/// Parse a CIDR notation string into an (address, prefix_length) tuple.
///
/// Returns `None` if the string is malformed or the prefix is out of range (>32).
pub fn parse_cidr(cidr: &str) -> Option<(Ipv4Addr, u8)> {
    let (ip_str, prefix_str) = cidr.split_once('/')?;
    let prefix: u8 = prefix_str.parse().ok()?;
    if prefix > 32 {
        return None;
    }
    let ip = parse_ipv4(ip_str)?;
    Some((ip, prefix))
}

/// Compute the network address by masking the given IP with the prefix length.
pub fn network_address(ip: Ipv4Addr, prefix: u8) -> Ipv4Addr {
    let raw = u32::from(ip);
    let mask = if prefix == 0 {
        0u32
    } else {
        !0u32 << (32 - prefix)
    };
    Ipv4Addr::from(raw & mask)
}

/// Expand a list of CIDR notation strings into sample IPv4 addresses.
///
/// For each CIDR, this generates up to `samples_per_cidr` IP addresses starting
/// from `network_address + 1`. The number of samples is clamped to stay within
/// the subnet (excluding the network address itself). Invalid CIDR strings are
/// skipped with a `tracing::warn!` — the function never panics.
pub fn expand_ipv4_cidrs(ranges: &[&str], samples_per_cidr: usize) -> Vec<Ipv4Addr> {
    let mut result = Vec::with_capacity(ranges.len() * samples_per_cidr);

    for cidr in ranges {
        let Some((ip, prefix)) = parse_cidr(cidr) else {
            tracing::warn!("Invalid CIDR notation, skipping: {cidr}");
            continue;
        };

        let network = network_address(ip, prefix);

        // Total addresses in this subnet: 2^(32-prefix)
        let total = 1u32 << (32 - prefix);
        // Maximum offset excluding the network address itself
        let max_offset = total.saturating_sub(1);
        let count = (samples_per_cidr as u32).min(max_offset);

        for offset in 1..=count {
            let raw = u32::from(network) + offset;
            result.push(Ipv4Addr::from(raw));
        }
    }

    result
}

// ── IPv6 CIDR helpers ──────────────────────────────────────────

/// Parse a single IPv6 CIDR string into (network address, prefix length).
///
/// Returns `None` if the string is malformed or the prefix is out of range (>128).
pub fn parse_ipv6_cidr(cidr: &str) -> Option<(Ipv6Addr, u8)> {
    let (ip_str, prefix_str) = cidr.split_once('/')?;
    let prefix: u8 = prefix_str.parse().ok()?;
    if prefix > 128 {
        return None;
    }
    let ip: Ipv6Addr = ip_str.parse().ok()?;
    Some((ip, prefix))
}

/// Compute the network address for an IPv6 CIDR.
pub fn ipv6_network_address(ip: Ipv6Addr, prefix: u8) -> Ipv6Addr {
    let raw = ip.to_bits();
    let mask = if prefix == 0 {
        0u128
    } else {
        !0u128 << (128 - prefix)
    };
    Ipv6Addr::from(raw & mask)
}

/// Expand IPv6 CIDR ranges into sample addresses.
///
/// For each CIDR, generates up to `samples_per_cidr` IPs starting from
/// `network_address + 1`. Samples are generated by incrementing the full
/// 128-bit address (wrapping is not a concern for the small sample counts used).
/// Invalid CIDR strings are skipped with a `tracing::warn!`.
pub fn expand_ipv6_cidrs(ranges: &[&str], samples_per_cidr: usize) -> Vec<Ipv6Addr> {
    let mut result = Vec::with_capacity(ranges.len() * samples_per_cidr);

    for cidr in ranges {
        let Some((ip, prefix)) = parse_ipv6_cidr(cidr) else {
            tracing::warn!("Invalid IPv6 CIDR notation, skipping: {cidr}");
            continue;
        };

        let network = ipv6_network_address(ip, prefix);
        let total = if prefix >= 128 { 0u128 } else { 1u128 << (128 - prefix) };
        let max_offset = total.saturating_sub(1);
        let count = (samples_per_cidr as u128).min(max_offset);

        for offset in 1..=count {
            result.push(Ipv6Addr::from(network.to_bits() + offset));
        }
    }

    result
}

// ── Cloudflare REST API ────────────────────────────────────────

const CLOUDFLARE_IPS_API: &str = "https://api.cloudflare.com/client/v4/ips";
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);

/// JSON response from `GET https://api.cloudflare.com/client/v4/ips`.
#[derive(Debug, Deserialize)]
struct CfIpsApiResponse {
    result: CfIpsResult,
}

#[derive(Debug, Deserialize)]
struct CfIpsResult {
    #[serde(default)]
    ipv4_cidrs: Vec<String>,
    #[serde(default)]
    ipv6_cidrs: Vec<String>,
}

/// Fetch Cloudflare IP ranges from the REST API (both v4 and v6 in one request).
///
/// Returns expanded IPv4 and IPv6 sample addresses plus the raw CIDR strings.
pub async fn fetch_cloudflare_ips(
) -> anyhow::Result<(Vec<Ipv4Addr>, Vec<Ipv6Addr>, Vec<String>, Vec<String>)> {
    let response = tokio::time::timeout(FETCH_TIMEOUT, reqwest::get(CLOUDFLARE_IPS_API))
        .await
        .map_err(|_| anyhow::anyhow!("fetch timed out after {}s", FETCH_TIMEOUT.as_secs()))?
        .map_err(|e| anyhow::anyhow!("HTTP request failed: {e}"))?;

    let body = response
        .text()
        .await
        .map_err(|e| anyhow::anyhow!("failed to read response body: {e}"))?;

    let api_resp: CfIpsApiResponse =
        serde_json::from_str(&body).map_err(|e| anyhow::anyhow!("failed to parse JSON: {e}"))?;

    let ipv4_cidrs = api_resp.result.ipv4_cidrs.clone();
    let ipv6_cidrs = api_resp.result.ipv6_cidrs.clone();

    let ipv4_cidr_strs: Vec<&str> = ipv4_cidrs.iter().map(|s| s.as_str()).collect();
    let ipv6_cidr_strs: Vec<&str> = ipv6_cidrs.iter().map(|s| s.as_str()).collect();

    let ipv4_addrs = expand_ipv4_cidrs(&ipv4_cidr_strs, 3);
    let ipv6_addrs = expand_ipv6_cidrs(&ipv6_cidr_strs, 3);

    Ok((ipv4_addrs, ipv6_addrs, ipv4_cidrs, ipv6_cidrs))
}

// ── CdnIpCache ─────────────────────────────────────────────────

/// Shared cache of Cloudflare IP ranges, used both by the CDN accelerator
/// for candidate generation and by [`super::resolver::is_cloudflare_domain`]
/// for domain detection.
#[derive(Clone, Debug)]
pub struct CdnIpCache {
    /// Expanded IPv4 probe addresses.
    pub ipv4_addrs: Vec<Ipv4Addr>,
    /// Expanded IPv6 probe addresses.
    pub ipv6_addrs: Vec<Ipv6Addr>,
    /// Raw IPv4 CIDR strings (for `is_cloudflare_domain` matching).
    pub ipv4_cidrs: Vec<String>,
    /// Raw IPv6 CIDR strings (for `is_cloudflare_domain` matching).
    pub ipv6_cidrs: Vec<String>,
    pub fetched_at: Instant,
    pub from_fallback: bool,
}

impl CdnIpCache {
    pub fn expired(&self) -> bool {
        (self.ipv4_addrs.is_empty() && self.ipv6_addrs.is_empty())
            || self.fetched_at.elapsed() >= CACHE_TTL
    }

    /// Return all candidate IPs as `Vec<IpAddr>`, v4 first then v6.
    pub fn all_addrs(&self) -> Vec<IpAddr> {
        let mut result = Vec::with_capacity(self.ipv4_addrs.len() + self.ipv6_addrs.len());
        result.extend(self.ipv4_addrs.iter().copied().map(IpAddr::V4));
        result.extend(self.ipv6_addrs.iter().copied().map(IpAddr::V6));
        result
    }

    /// Create a cache populated from static fallback CIDR ranges.
    pub fn from_fallback() -> Self {
        let ipv4_cidrs: Vec<String> = CLOUDFLARE_IPV4_RANGES.iter().map(|s| s.to_string()).collect();
        let ipv6_cidrs: Vec<String> = CLOUDFLARE_IPV6_RANGES.iter().map(|s| s.to_string()).collect();
        let ipv4_addrs = expand_ipv4_cidrs(CLOUDFLARE_IPV4_RANGES, 3);
        let ipv6_addrs = expand_ipv6_cidrs(CLOUDFLARE_IPV6_RANGES, 3);
        Self {
            ipv4_addrs,
            ipv6_addrs,
            ipv4_cidrs,
            ipv6_cidrs,
            fetched_at: Instant::now(),
            from_fallback: true,
        }
    }
}

/// Fetch and return a fresh or cached [`CdnIpCache`].
///
/// Uses a three-tier strategy:
/// 1. Cache hit (< 24h, non-empty) → return immediately
/// 2. Stale cache with data → return stale, refresh in background
/// 3. Empty cache → must fetch; cancellation triggers static fallback
pub async fn get_ip_ranges(
    cache: Arc<Mutex<CdnIpCache>>,
    cancel: CancellationToken,
) -> CdnIpCache {
    // Fast path: cache is valid (<24h, non-empty) — return immediately.
    {
        let cached = cache.lock().await;
        if !cached.expired() {
            return cached.clone();
        }
    }

    // Stale path: cache expired but has data — return stale, refresh in background.
    {
        let cached = cache.lock().await;
        if !cached.ipv4_addrs.is_empty() || !cached.ipv6_addrs.is_empty() {
            let stale = cached.clone();
            drop(cached);

            let cache_clone = cache.clone();
            tokio::spawn(async move {
                match fetch_cloudflare_ips().await {
                    Ok((ipv4_addrs, ipv6_addrs, ipv4_cidrs, ipv6_cidrs)) => {
                        tracing::info!(
                            v4 = ipv4_addrs.len(),
                            v6 = ipv6_addrs.len(),
                            "Background Cloudflare IP refresh succeeded"
                        );
                        let mut cached = cache_clone.lock().await;
                        *cached = CdnIpCache {
                            ipv4_addrs,
                            ipv6_addrs,
                            ipv4_cidrs,
                            ipv6_cidrs,
                            fetched_at: Instant::now(),
                            from_fallback: false,
                        };
                    }
                    Err(e) => {
                        tracing::warn!("Background Cloudflare IP refresh failed: {e}");
                    }
                }
            });

            return stale;
        }
    }

    // Empty cache: must fetch. Respect cancellation via tokio::select!.
    let result = tokio::select! {
        _ = cancel.cancelled() => {
            tracing::warn!("IP range fetch cancelled, using static fallback");
            let fb = CdnIpCache::from_fallback();
            let mut cached = cache.lock().await;
            *cached = fb.clone();
            return fb;
        }
        result = fetch_cloudflare_ips() => result,
    };

    match result {
        Ok((ipv4_addrs, ipv6_addrs, ipv4_cidrs, ipv6_cidrs)) => {
            let cached_data = CdnIpCache {
                ipv4_addrs,
                ipv6_addrs,
                ipv4_cidrs,
                ipv6_cidrs,
                fetched_at: Instant::now(),
                from_fallback: false,
            };
            let mut cached = cache.lock().await;
            *cached = cached_data.clone();
            cached_data
        }
        Err(e) => {
            tracing::warn!("Failed to fetch Cloudflare IP ranges, using static fallback: {e}");
            let fb = CdnIpCache::from_fallback();
            let mut cached = cache.lock().await;
            *cached = fb.clone();
            fb
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_util::sync::CancellationToken;

    // ── IPv4 tests ──────────────────────────────────────────────

    #[test]
    fn test_expand_cidrs() {
        let ips = expand_ipv4_cidrs(CLOUDFLARE_IPV4_RANGES, 3);

        // 15 CIDRs × 3 samples = 45 IPs
        assert_eq!(ips.len(), 45, "Expected 45 IPs from 15 CIDRs × 3 samples");

        // Verify first 3 IPs from first CIDR (173.245.48.0/20)
        assert_eq!(ips[0], Ipv4Addr::new(173, 245, 48, 1));
        assert_eq!(ips[1], Ipv4Addr::new(173, 245, 48, 2));
        assert_eq!(ips[2], Ipv4Addr::new(173, 245, 48, 3));

        for ip in &ips {
            assert_ne!(*ip, Ipv4Addr::UNSPECIFIED, "Sample IP must not be 0.0.0.0");
        }
    }

    #[test]
    fn test_parse_cidr_invalid() {
        assert!(parse_cidr("not-a-cidr").is_none());
        assert!(parse_cidr("256.0.0.0/24").is_none());
        assert!(parse_cidr("1.2.3.4/33").is_none());
        assert!(parse_cidr("1.2.3.4/abc").is_none());
        assert!(parse_cidr("1.2.3.4").is_none());
    }

    #[test]
    fn test_expand_clamped_32() {
        let ips = expand_ipv4_cidrs(&["192.0.2.1/32"], 5);
        assert!(
            ips.is_empty(),
            "/32 should yield 0 samples (only network address)"
        );
    }

    #[test]
    fn test_expand_clamped_31() {
        let ips = expand_ipv4_cidrs(&["192.0.2.0/31"], 5);
        assert_eq!(ips.len(), 1, "/31 should yield at most 1 sample");
        assert_eq!(ips[0], Ipv4Addr::new(192, 0, 2, 1));
    }

    #[test]
    fn test_static_fallback_bundle_size() {
        let ips = expand_ipv4_cidrs(CLOUDFLARE_IPV4_RANGES, 3);
        assert_eq!(
            ips.len(),
            45,
            "static fallback must produce 45 IPs (15 CIDRs × 3)"
        );
    }

    // ── IPv6 tests ──────────────────────────────────────────────

    #[test]
    fn test_parse_ipv6_cidr_valid() {
        let (net, prefix) = parse_ipv6_cidr("2606:4700::/32").unwrap();
        assert_eq!(net, Ipv6Addr::new(0x2606, 0x4700, 0, 0, 0, 0, 0, 0));
        assert_eq!(prefix, 32);
    }

    #[test]
    fn test_parse_ipv6_cidr_invalid() {
        assert!(parse_ipv6_cidr("not-a-cidr").is_none());
        assert!(parse_ipv6_cidr("::1/129").is_none());
        assert!(parse_ipv6_cidr("::1/abc").is_none());
        assert!(parse_ipv6_cidr("::1").is_none());
        // Out-of-range segment
        assert!(parse_ipv6_cidr("gggg::1/32").is_none());
    }

    #[test]
    fn test_ipv6_network_address() {
        // 2606:4700:1234::/32 → network is 2606:4700::
        let ip = Ipv6Addr::new(0x2606, 0x4700, 0x1234, 0, 0, 0, 0, 0);
        let network = ipv6_network_address(ip, 32);
        assert_eq!(network, Ipv6Addr::new(0x2606, 0x4700, 0, 0, 0, 0, 0, 0));
    }

    #[test]
    fn test_expand_ipv6_cidrs() {
        let ips = expand_ipv6_cidrs(CLOUDFLARE_IPV6_RANGES, 3);
        // 6 CIDRs × 3 samples = 18 IPs
        assert_eq!(ips.len(), 18, "Expected 18 IPs from 6 CIDRs × 3 samples");

        // First IP from 2606:4700::/32 should be 2606:4700::1
        assert_eq!(ips[0], Ipv6Addr::new(0x2606, 0x4700, 0, 0, 0, 0, 0, 1));
        assert_eq!(ips[1], Ipv6Addr::new(0x2606, 0x4700, 0, 0, 0, 0, 0, 2));
        assert_eq!(ips[2], Ipv6Addr::new(0x2606, 0x4700, 0, 0, 0, 0, 0, 3));
    }

    #[test]
    fn test_expand_ipv6_cidrs_clamped_128() {
        // /128 has exactly 1 address — no room for host samples
        let ips = expand_ipv6_cidrs(&["::1/128"], 5);
        assert!(ips.is_empty(), "/128 should yield 0 samples");
    }

    // ── CdnIpCache tests ────────────────────────────────────────

    #[test]
    fn test_all_addrs_mixed() {
        let cache = CdnIpCache::from_fallback();
        let all = cache.all_addrs();
        // 45 IPv4 + 18 IPv6 = 63 total
        assert_eq!(all.len(), 63);
        // First addresses should be IPv4
        assert!(matches!(all[0], IpAddr::V4(_)));
        // IPv6 should appear after IPv4
        assert!(matches!(all[45], IpAddr::V6(_)));
    }

    #[test]
    fn test_cache_fallback_not_expired() {
        let cache = CdnIpCache {
            ipv4_addrs: vec![Ipv4Addr::new(1, 1, 1, 1)],
            ipv6_addrs: vec![],
            ipv4_cidrs: vec![],
            ipv6_cidrs: vec![],
            fetched_at: Instant::now(),
            from_fallback: true,
        };
        assert!(!cache.expired());
    }

    #[test]
    fn test_cache_empty_is_expired() {
        let cache = CdnIpCache {
            ipv4_addrs: vec![],
            ipv6_addrs: vec![],
            ipv4_cidrs: vec![],
            ipv6_cidrs: vec![],
            fetched_at: Instant::now(),
            from_fallback: true,
        };
        assert!(cache.expired(), "empty cache must be considered expired");
    }

    #[test]
    fn test_cache_ttl_expired() {
        if let Some(old) = Instant::now().checked_sub(CACHE_TTL + Duration::from_secs(3600)) {
            let cache = CdnIpCache {
                ipv4_addrs: vec![Ipv4Addr::new(1, 1, 1, 1)],
                ipv6_addrs: vec![],
                ipv4_cidrs: vec![],
                ipv6_cidrs: vec![],
                fetched_at: old,
                from_fallback: false,
            };
            assert!(cache.expired(), "cache older than CACHE_TTL must be expired");
        }
    }

    // ── async tests ─────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_ip_ranges_cancellation() {
        let cache = Arc::new(Mutex::new(CdnIpCache {
            ipv4_addrs: Vec::new(),
            ipv6_addrs: Vec::new(),
            ipv4_cidrs: Vec::new(),
            ipv6_cidrs: Vec::new(),
            fetched_at: Instant::now(),
            from_fallback: true,
        }));

        let cancel = CancellationToken::new();
        cancel.cancel();

        let result = get_ip_ranges(Arc::clone(&cache), cancel).await;
        assert_eq!(result.ipv4_addrs.len(), 45, "cancelled fetch must return 45 IPv4 from fallback");
        assert_eq!(result.ipv6_addrs.len(), 18, "cancelled fetch must return 18 IPv6 from fallback");
        assert!(result.from_fallback);
    }

    #[tokio::test]
    async fn test_get_ip_ranges_returns_valid_cache() {
        let cache = Arc::new(Mutex::new(CdnIpCache {
            ipv4_addrs: vec![Ipv4Addr::new(10, 0, 0, 1), Ipv4Addr::new(10, 0, 0, 2)],
            ipv6_addrs: vec![],
            ipv4_cidrs: vec!["10.0.0.0/8".into()],
            ipv6_cidrs: vec![],
            fetched_at: Instant::now(),
            from_fallback: false,
        }));

        // Fresh, non-empty cache must return immediately with cached data
        let result = get_ip_ranges(Arc::clone(&cache), CancellationToken::new()).await;
        assert_eq!(result.ipv4_addrs.len(), 2);
        assert_eq!(result.ipv4_addrs[0], Ipv4Addr::new(10, 0, 0, 1));
        assert_eq!(result.ipv4_addrs[1], Ipv4Addr::new(10, 0, 0, 2));
        assert!(!result.from_fallback);
    }

    #[tokio::test]
    async fn test_get_ip_ranges_stale_refresh() {
        // Create a stale (expired) but non-empty cache
        let old_time = Instant::now()
            .checked_sub(CACHE_TTL + Duration::from_secs(3600))
            .unwrap_or(Instant::now());
        let cache = Arc::new(Mutex::new(CdnIpCache {
            ipv4_addrs: vec![Ipv4Addr::new(1, 1, 1, 1)],
            ipv6_addrs: vec![],
            ipv4_cidrs: vec!["1.1.1.0/24".into()],
            ipv6_cidrs: vec![],
            fetched_at: old_time,
            from_fallback: true,
        }));

        // Should return stale data immediately (not block on network fetch)
        let result = get_ip_ranges(Arc::clone(&cache), CancellationToken::new()).await;
        assert_eq!(result.ipv4_addrs.len(), 1);
        assert_eq!(result.ipv4_addrs[0], Ipv4Addr::new(1, 1, 1, 1));
    }

    #[test]
    fn test_fallback_cache_is_valid() {
        let fb = &*FALLBACK_CACHE;
        assert_eq!(fb.ipv4_addrs.len(), 45);
        assert_eq!(fb.ipv6_addrs.len(), 18);
        assert_eq!(fb.ipv4_cidrs.len(), 15);
        assert_eq!(fb.ipv6_cidrs.len(), 6);
        assert!(fb.from_fallback);
    }
}
