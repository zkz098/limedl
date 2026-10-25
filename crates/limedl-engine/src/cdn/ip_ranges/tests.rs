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
        assert!(
            cache.expired(),
            "cache older than CACHE_TTL must be expired"
        );
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
    assert_eq!(
        result.ipv4_addrs.len(),
        45,
        "cancelled fetch must return 45 IPv4 from fallback"
    );
    assert_eq!(
        result.ipv6_addrs.len(),
        18,
        "cancelled fetch must return 18 IPv6 from fallback"
    );
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
