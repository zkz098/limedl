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

    let ipv4_cidr_strs: Vec<&str> = ipv4_cidrs.iter().map(String::as_str).collect();
    let ipv6_cidr_strs: Vec<&str> = ipv6_cidrs.iter().map(String::as_str).collect();

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
        let ipv4_cidrs: Vec<String> = CLOUDFLARE_IPV4_RANGES.iter().map(ToString::to_string).collect();
        let ipv6_cidrs: Vec<String> = CLOUDFLARE_IPV6_RANGES.iter().map(ToString::to_string).collect();
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
mod tests;
