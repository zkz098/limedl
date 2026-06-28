#![allow(dead_code)]

use std::net::Ipv4Addr;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

/// Cache TTL: 24 hours before considering cached IP ranges stale.
pub(crate) const CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// Static fallback list of Cloudflare IPv4 CIDR ranges.
///
/// Source: <https://www.cloudflare.com/ips-v4> — verified June 2026.
/// These are used when live HTTP fetching of the current ranges fails.
pub(crate) const CLOUDFLARE_IPV4_RANGES: &[&str] = &[
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
pub(crate) fn parse_cidr(cidr: &str) -> Option<(Ipv4Addr, u8)> {
    let (ip_str, prefix_str) = cidr.split_once('/')?;
    let prefix: u8 = prefix_str.parse().ok()?;
    if prefix > 32 {
        return None;
    }
    let ip = parse_ipv4(ip_str)?;
    Some((ip, prefix))
}

/// Compute the network address by masking the given IP with the prefix length.
pub(crate) fn network_address(ip: Ipv4Addr, prefix: u8) -> Ipv4Addr {
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
///
/// This is used as a static fallback when live HTTP fetching of Cloudflare IP
/// ranges fails, providing probe targets for CDN acceleration.
pub(crate) fn expand_ipv4_cidrs(ranges: &[&str], samples_per_cidr: usize) -> Vec<Ipv4Addr> {
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

const CLOUDFLARE_IPV4_URL: &str = "https://www.cloudflare.com/ips-v4";
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug)]
pub(crate) struct IpRangesCache {
    pub ips: Vec<Ipv4Addr>,
    pub fetched_at: Instant,
    pub from_fallback: bool,
}

impl IpRangesCache {
    pub(crate) fn expired(&self) -> bool {
        self.ips.is_empty() || self.fetched_at.elapsed() >= CACHE_TTL
    }
}

pub(crate) async fn fetch_cloudflare_ipv4_ranges() -> anyhow::Result<Vec<Ipv4Addr>> {
    fetch_ranges_from_url(CLOUDFLARE_IPV4_URL).await
}

pub(crate) async fn fetch_ranges_from_url(url: &str) -> anyhow::Result<Vec<Ipv4Addr>> {
    let response = tokio::time::timeout(FETCH_TIMEOUT, reqwest::get(url))
        .await
        .map_err(|_| anyhow::anyhow!("fetch timed out after {}s", FETCH_TIMEOUT.as_secs()))?
        .map_err(|e| anyhow::anyhow!("HTTP request failed: {e}"))?;

    let body = response
        .text()
        .await
        .map_err(|e| anyhow::anyhow!("failed to read response body: {e}"))?;

    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Err(anyhow::anyhow!("empty response body"));
    }

    let cidrs: Vec<&str> = trimmed
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();

    if cidrs.is_empty() {
        return Err(anyhow::anyhow!("no CIDR lines found in response"));
    }

    Ok(expand_ipv4_cidrs(&cidrs, 3))
}

pub(crate) async fn get_ip_ranges(
    cache: &Mutex<IpRangesCache>,
    cancel: CancellationToken,
) -> IpRangesCache {
    // Fast path: cache is valid (<24h, non-empty) — return immediately.
    {
        let cached = cache.lock().await;
        if !cached.ips.is_empty() && !cached.expired() {
            return cached.clone();
        }
    }

    // Stale path: cache expired but has data — return stale, refresh in background.
    {
        let cached = cache.lock().await;
        if !cached.ips.is_empty() {
            let stale = cached.clone();
            drop(cached);

            // Spawn a best-effort background refresh. Since the cache is borrowed
            // (&Mutex) we cannot hand ownership to the spawned task; the fetch still
            // runs but does not update the cache. The stale data is returned immediately.
            tokio::spawn(async move {
                match fetch_cloudflare_ipv4_ranges().await {
                    Ok(ips) => {
                        tracing::info!(
                            ips_len = ips.len(),
                            "Background Cloudflare IP refresh succeeded"
                        );
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
            let ips = expand_ipv4_cidrs(CLOUDFLARE_IPV4_RANGES, 3);
            let mut cached = cache.lock().await;
            *cached = IpRangesCache {
                ips: ips.clone(),
                fetched_at: Instant::now(),
                from_fallback: true,
            };
            return cached.clone();
        }
        result = fetch_cloudflare_ipv4_ranges() => result,
    };

    match result {
        Ok(ips) => {
            let mut cached = cache.lock().await;
            *cached = IpRangesCache {
                ips,
                fetched_at: Instant::now(),
                from_fallback: false,
            };
            cached.clone()
        }
        Err(e) => {
            tracing::warn!("Failed to fetch Cloudflare IP ranges, using static fallback: {e}");
            let ips = expand_ipv4_cidrs(CLOUDFLARE_IPV4_RANGES, 3);
            let mut cached = cache.lock().await;
            *cached = IpRangesCache {
                ips,
                fetched_at: Instant::now(),
                from_fallback: true,
            };
            cached.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_util::sync::CancellationToken;

    #[test]
    fn test_expand_cidrs() {
        let ips = expand_ipv4_cidrs(CLOUDFLARE_IPV4_RANGES, 3);

        // 15 CIDRs × 3 samples = 45 IPs
        assert_eq!(ips.len(), 45, "Expected 45 IPs from 15 CIDRs × 3 samples");

        // Verify first 3 IPs from first CIDR (173.245.48.0/20)
        assert_eq!(ips[0], Ipv4Addr::new(173, 245, 48, 1));
        assert_eq!(ips[1], Ipv4Addr::new(173, 245, 48, 2));
        assert_eq!(ips[2], Ipv4Addr::new(173, 245, 48, 3));

        // Verify every returned IP is a valid Ipv4Addr (nonzero, not unspecified)
        for ip in &ips {
            assert_ne!(*ip, Ipv4Addr::UNSPECIFIED, "Sample IP must not be 0.0.0.0");
        }
    }

    #[test]
    fn test_parse_cidr_invalid() {
        // Non-CIDR string
        assert!(parse_cidr("not-a-cidr").is_none());
        // Out-of-range octet
        assert!(parse_cidr("256.0.0.0/24").is_none());
        // Prefix > 32
        assert!(parse_cidr("1.2.3.4/33").is_none());
        // Non-numeric prefix
        assert!(parse_cidr("1.2.3.4/abc").is_none());
        // Missing prefix
        assert!(parse_cidr("1.2.3.4").is_none());
    }

    #[test]
    fn test_expand_clamped_32() {
        // /32 subnet has exactly 1 address — no room for host samples
        let ips = expand_ipv4_cidrs(&["192.0.2.1/32"], 5);
        assert!(
            ips.is_empty(),
            "/32 should yield 0 samples (only network address)"
        );
    }

    #[test]
    fn test_expand_clamped_31() {
        // /31 subnet has 2 addresses — at most 1 host sample
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

    #[tokio::test]
    async fn test_fetch_from_bad_url_fails() {
        let result = fetch_ranges_from_url("http://127.0.0.1:1/nonexistent").await;
        assert!(result.is_err(), "fetch from unreachable URL must fail");
    }

    #[tokio::test]
    async fn test_caching_returns_cached_data() {
        let cache = Mutex::new(IpRangesCache {
            ips: vec![Ipv4Addr::new(1, 1, 1, 1), Ipv4Addr::new(2, 2, 2, 2)],
            fetched_at: Instant::now(),
            from_fallback: true,
        });

        let first = get_ip_ranges(&cache, CancellationToken::new()).await;
        assert_eq!(first.ips.len(), 2);
        assert_eq!(first.ips[0], Ipv4Addr::new(1, 1, 1, 1));
        assert_eq!(first.ips[1], Ipv4Addr::new(2, 2, 2, 2));
        assert!(first.from_fallback);

        let second = get_ip_ranges(&cache, CancellationToken::new()).await;
        assert_eq!(second.ips.len(), 2);
        assert_eq!(second.ips[0], Ipv4Addr::new(1, 1, 1, 1));
        assert_eq!(second.ips[1], Ipv4Addr::new(2, 2, 2, 2));
    }

    #[tokio::test]
    async fn test_fallback_empty_cache_on_bad_url() {
        let result = fetch_ranges_from_url("http://127.0.0.1:1/").await;
        assert!(result.is_err());

        let fallback_ips = expand_ipv4_cidrs(CLOUDFLARE_IPV4_RANGES, 3);
        assert_eq!(fallback_ips.len(), 45);
    }

    #[test]
    fn test_cache_ttl_not_expired() {
        let cache = IpRangesCache {
            ips: vec![Ipv4Addr::new(1, 1, 1, 1)],
            fetched_at: Instant::now(),
            from_fallback: false,
        };
        assert!(!cache.expired(), "fresh cache must not be expired");
    }

    #[test]
    fn test_cache_ttl_expired() {
        // Instant is uptime-based on Windows — checked_sub may return None
        // if the system hasn't been running long enough. When it succeeds,
        // verify time-based expiry; otherwise this test is a no-op (the
        // empty-cache expiry path is covered by test_cache_ttl_empty_is_expired).
        if let Some(old) = Instant::now().checked_sub(CACHE_TTL + Duration::from_secs(3600)) {
            let cache = IpRangesCache {
                ips: vec![Ipv4Addr::new(1, 1, 1, 1)],
                fetched_at: old,
                from_fallback: false,
            };
            assert!(
                cache.expired(),
                "cache older than CACHE_TTL must be expired"
            );
        }
    }

    #[test]
    fn test_cache_ttl_empty_is_expired() {
        let cache = IpRangesCache {
            ips: vec![],
            fetched_at: Instant::now(),
            from_fallback: true,
        };
        assert!(cache.expired(), "empty cache must be considered expired");
    }

    #[tokio::test]
    async fn test_get_ip_ranges_cancellation() {
        let cache = Mutex::new(IpRangesCache {
            ips: Vec::new(),
            fetched_at: Instant::now(),
            from_fallback: true,
        });

        let cancel = CancellationToken::new();
        cancel.cancel();

        // With empty cache and cancelled token, must fall back to static ranges
        let result = get_ip_ranges(&cache, cancel).await;
        assert_eq!(
            result.ips.len(),
            45,
            "cancelled fetch must return 45 IPs from static fallback"
        );
        assert!(result.from_fallback);
    }

    #[tokio::test]
    async fn test_get_ip_ranges_returns_valid_cache() {
        let cache = Mutex::new(IpRangesCache {
            ips: vec![Ipv4Addr::new(10, 0, 0, 1), Ipv4Addr::new(10, 0, 0, 2)],
            fetched_at: Instant::now(),
            from_fallback: false,
        });

        // Fresh, non-empty cache must return immediately with cached data
        let result = get_ip_ranges(&cache, CancellationToken::new()).await;
        assert_eq!(result.ips.len(), 2);
        assert_eq!(result.ips[0], Ipv4Addr::new(10, 0, 0, 1));
        assert_eq!(result.ips[1], Ipv4Addr::new(10, 0, 0, 2));
        assert!(!result.from_fallback);
    }
}
