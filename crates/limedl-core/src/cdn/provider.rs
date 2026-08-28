use std::net::IpAddr;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use super::ip_ranges::{
    self, CdnIpCache, CLOUDFLARE_IPV4_RANGES, CLOUDFLARE_IPV6_RANGES,
};
use super::resolver::{ip_in_cloudflare_ipv6_ranges, ip_in_cloudflare_ranges};

/// Static fallback CIDR ranges for Fastly.
pub const FASTLY_IPV4_RANGES: &[&str] = &[
    "151.101.0.0/16",
    "199.232.0.0/16",
    "146.75.0.0/16",
    "167.82.0.0/16",
    "199.27.72.0/21",
];

/// Static fallback IPv6 CIDR ranges for Fastly.
pub const FASTLY_IPV6_RANGES: &[&str] = &[
    "2a04:4e40::/32",
    "2a04:4e42::/32",
];

/// Supported CDN Provider kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CdnProviderKind {
    Cloudflare,
    Fastly,
    Custom,
}

/// Extensible interface for CDN Providers.
///
/// Each provider supplies its Anycast IP ranges, default speed-test URL,
/// and logic to identify whether a given IP or hostname belongs to its network.
#[async_trait::async_trait]
pub trait CdnProvider: Send + Sync + 'static {
    /// Provider kind identifier.
    fn kind(&self) -> CdnProviderKind;

    /// Human-readable display name.
    fn name(&self) -> &str;

    /// Default throughput test URL.
    fn default_test_url(&self) -> &str;

    /// Fallback static IPv4 CIDR ranges.
    fn fallback_ipv4_cidrs(&self) -> &'static [&'static str];

    /// Fallback static IPv6 CIDR ranges.
    fn fallback_ipv6_cidrs(&self) -> &'static [&'static str];

    /// Build a cache populated from static fallback CIDRs.
    fn fallback_cache(&self) -> CdnIpCache {
        let ipv4_cidrs: Vec<String> = self
            .fallback_ipv4_cidrs()
            .iter()
            .map(|s| s.to_string())
            .collect();
        let ipv6_cidrs: Vec<String> = self
            .fallback_ipv6_cidrs()
            .iter()
            .map(|s| s.to_string())
            .collect();
        let ipv4_addrs = ip_ranges::expand_ipv4_cidrs(self.fallback_ipv4_cidrs(), 3);
        let ipv6_addrs = ip_ranges::expand_ipv6_cidrs(self.fallback_ipv6_cidrs(), 3);
        CdnIpCache {
            ipv4_addrs,
            ipv6_addrs,
            ipv4_cidrs,
            ipv6_cidrs,
            fetched_at: std::time::Instant::now(),
            from_fallback: true,
        }
    }

    /// Fetch fresh IP ranges from dynamic APIs or return fallback cache.
    async fn fetch_ip_ranges(&self, cancel: CancellationToken) -> CdnIpCache;

    /// Check if a resolved IP address belongs to this provider.
    fn matches_ip(&self, cache: &CdnIpCache, ip: IpAddr) -> bool {
        match ip {
            IpAddr::V4(v4) => ip_in_cloudflare_ranges(v4, &cache.ipv4_cidrs),
            IpAddr::V6(v6) => ip_in_cloudflare_ipv6_ranges(v6, &cache.ipv6_cidrs),
        }
    }
}

/// Official Cloudflare Anycast Provider.
#[derive(Debug, Default, Clone)]
pub struct CloudflareProvider;

#[async_trait::async_trait]
impl CdnProvider for CloudflareProvider {
    fn kind(&self) -> CdnProviderKind {
        CdnProviderKind::Cloudflare
    }

    fn name(&self) -> &'static str {
        "Cloudflare"
    }

    fn default_test_url(&self) -> &'static str {
        "https://speed.cloudflare.com/__down?bytes=25000000"
    }

    fn fallback_ipv4_cidrs(&self) -> &'static [&'static str] {
        CLOUDFLARE_IPV4_RANGES
    }

    fn fallback_ipv6_cidrs(&self) -> &'static [&'static str] {
        CLOUDFLARE_IPV6_RANGES
    }

    async fn fetch_ip_ranges(&self, cancel: CancellationToken) -> CdnIpCache {
        let cache = Arc::new(tokio::sync::Mutex::new(CdnIpCache::from_fallback()));
        ip_ranges::get_ip_ranges(cache, cancel).await
    }
}

/// Fastly Anycast Provider.
#[derive(Debug, Default, Clone)]
pub struct FastlyProvider;

#[async_trait::async_trait]
impl CdnProvider for FastlyProvider {
    fn kind(&self) -> CdnProviderKind {
        CdnProviderKind::Fastly
    }

    fn name(&self) -> &'static str {
        "Fastly"
    }

    fn default_test_url(&self) -> &'static str {
        "https://fastly-speedtest.com/download/25mb"
    }

    fn fallback_ipv4_cidrs(&self) -> &'static [&'static str] {
        FASTLY_IPV4_RANGES
    }

    fn fallback_ipv6_cidrs(&self) -> &'static [&'static str] {
        FASTLY_IPV6_RANGES
    }

    async fn fetch_ip_ranges(&self, _cancel: CancellationToken) -> CdnIpCache {
        self.fallback_cache()
    }
}

/// Custom User-Configured CDN Provider.
#[derive(Debug, Clone)]
pub struct CustomCdnProvider {
    name: String,
    test_url: String,
    ipv4_cidrs: Vec<String>,
    ipv6_cidrs: Vec<String>,
}

impl CustomCdnProvider {
    pub fn new(
        name: impl Into<String>,
        test_url: impl Into<String>,
        ipv4_cidrs: Vec<String>,
        ipv6_cidrs: Vec<String>,
    ) -> Self {
        Self {
            name: name.into(),
            test_url: test_url.into(),
            ipv4_cidrs,
            ipv6_cidrs,
        }
    }
}

#[async_trait::async_trait]
impl CdnProvider for CustomCdnProvider {
    fn kind(&self) -> CdnProviderKind {
        CdnProviderKind::Custom
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn default_test_url(&self) -> &str {
        &self.test_url
    }

    fn fallback_ipv4_cidrs(&self) -> &'static [&'static str] {
        &[]
    }

    fn fallback_ipv6_cidrs(&self) -> &'static [&'static str] {
        &[]
    }

    fn fallback_cache(&self) -> CdnIpCache {
        let ipv4_strs: Vec<&str> = self.ipv4_cidrs.iter().map(|s| s.as_str()).collect();
        let ipv6_strs: Vec<&str> = self.ipv6_cidrs.iter().map(|s| s.as_str()).collect();
        let ipv4_addrs = ip_ranges::expand_ipv4_cidrs(&ipv4_strs, 3);
        let ipv6_addrs = ip_ranges::expand_ipv6_cidrs(&ipv6_strs, 3);
        CdnIpCache {
            ipv4_addrs,
            ipv6_addrs,
            ipv4_cidrs: self.ipv4_cidrs.clone(),
            ipv6_cidrs: self.ipv6_cidrs.clone(),
            fetched_at: std::time::Instant::now(),
            from_fallback: true,
        }
    }

    async fn fetch_ip_ranges(&self, _cancel: CancellationToken) -> CdnIpCache {
        self.fallback_cache()
    }
}
