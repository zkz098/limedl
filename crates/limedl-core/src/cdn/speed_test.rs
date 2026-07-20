#![allow(dead_code)]

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use reqwest::{Client, Url};
use tokio::net::TcpStream;
use tokio::task::JoinSet;
use tokio::time::timeout;

use crate::error::{DownloadError, Result};
use crate::http_client_factory::configure_client_builder;
use crate::types::{AppSettings, default_http_user_agent};

/// Cloudflare CDN speed test endpoint (~100MB file).
/// Cloudflare rejects requests for files larger than 99_999_999 bytes with HTTP 403.
pub const SPEED_TEST_URL: &str = "https://speed.cloudflare.com/__down?bytes=99999999";

/// Maximum duration for a single IP's throughput test.
pub const SPEED_TEST_DURATION: Duration = Duration::from_secs(10);

// ── Progress reporting types ──────────────────────────────────

/// Phases of the CDN speed test. Frontend consumes these as camelCase strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CdnTestPhase {
    FetchingRanges,
    Screening,
    MeasuringThroughput,
}

/// Progress snapshot emitted to the frontend during a CDN speed test.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CdnTestProgress {
    pub phase: CdnTestPhase,
    pub current: u64,
    pub total: u64,
}

/// Erased progress callback for reporting phase transitions and per-IP progress.
pub type ProgressFn = Box<dyn Fn(CdnTestPhase, u64, u64) + Send + Sync>;

// ── Orchestrator types ────────────────────────────────────────

/// Configuration for the two-phase speed test orchestrator.
#[derive(Debug, Clone)]
pub struct SpeedTestConfig {
    /// Max concurrent TCP connections during screening.
    pub concurrency: usize,
    /// Per-IP TCP connect timeout.
    pub tcp_timeout: Duration,
    /// Max duration for a single IP's throughput test.
    pub throughput_duration: Duration,
    /// Number of fastest IPs (by TCP latency) to advance to Phase 2.
    pub top_n_candidates: usize,
}

impl Default for SpeedTestConfig {
    fn default() -> Self {
        Self {
            concurrency: 50,
            tcp_timeout: Duration::from_secs(3),
            throughput_duration: Duration::from_secs(10),
            top_n_candidates: 5,
        }
    }
}

/// Result for a single IP after the two-phase speed test.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedTestResult {
    pub ip: Ipv4Addr,
    /// TCP connect latency in milliseconds.
    pub tcp_latency_ms: f64,
    /// Throughput in MB/s (bytes / seconds / 1_000_000), or None if Phase 2 failed.
    pub throughput_mbps: Option<f64>,
    /// Error message from Phase 2, if any.
    pub error: Option<String>,
}

impl Default for SpeedTestResult {
    fn default() -> Self {
        Self {
            ip: Ipv4Addr::UNSPECIFIED,
            tcp_latency_ms: 0.0,
            throughput_mbps: None,
            error: None,
        }
    }
}

/// Baseline measurement of the default DNS-resolved node (no IP override).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultNodeResult {
    pub ip: Option<String>,
    pub tcp_latency_ms: f64,
    pub throughput_mbps: Option<f64>,
    pub error: Option<String>,
}

/// Measure the default DNS-resolved node for `speed.cloudflare.com`.
///
/// Resolves the hostname via standard DNS (no IP override), measures TCP latency
/// to the first resolved IPv4, then measures download throughput with a normal
/// client. This provides the "unoptimized" baseline for comparison.
pub async fn measure_default_node(settings: &AppSettings) -> DefaultNodeResult {
    const HOSTNAME: &str = "speed.cloudflare.com";

    tracing::info!("default node: measuring baseline for {HOSTNAME}");

    let resolved_ip = match tokio::net::lookup_host(format!("{HOSTNAME}:443")).await {
        Ok(addrs) => addrs.into_iter().find_map(|a| match a.ip() {
            std::net::IpAddr::V4(v4) => Some(v4),
            _ => None,
        }),
        Err(e) => {
            tracing::warn!("default node: DNS resolution failed: {e}");
            return DefaultNodeResult {
                ip: None,
                tcp_latency_ms: 0.0,
                throughput_mbps: None,
                error: Some(format!("DNS resolution failed: {e}")),
            };
        }
    };

    let ip = match resolved_ip {
        Some(ip) => ip,
        None => {
            tracing::warn!("default node: no IPv4 address resolved");
            return DefaultNodeResult {
                ip: None,
                tcp_latency_ms: 0.0,
                throughput_mbps: None,
                error: Some("no IPv4 address resolved".into()),
            };
        }
    };

    tracing::info!("default node: DNS resolved to {ip}");

    let latency =
        measure_tcp_latency(SocketAddr::new(IpAddr::V4(ip), 443), Duration::from_secs(5)).await;

    let tcp_latency_ms = latency.map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0);

    let throughput_result = measure_throughput(ip, HOSTNAME, SPEED_TEST_URL, settings).await;

    let (throughput_mbps, error) = match throughput_result {
        Ok((bytes, elapsed_ms)) => {
            let elapsed_secs = elapsed_ms as f64 / 1000.0;
            let mbps = if elapsed_secs > 0.0 {
                Some((bytes / elapsed_secs) / 1_000_000.0)
            } else {
                None
            };
            tracing::info!(
                "default node: throughput={:.2} MB/s latency={tcp_latency_ms:.1}ms ip={ip}",
                mbps.unwrap_or(0.0),
            );
            (mbps, None)
        }
        Err(e) => {
            tracing::warn!("default node: throughput test failed: {e}");
            (None, Some(e.to_string()))
        }
    };

    DefaultNodeResult {
        ip: Some(ip.to_string()),
        tcp_latency_ms,
        throughput_mbps,
        error,
    }
}

/// Build a throwaway `reqwest::Client` with DNS-override and settings mirrored
/// from the main `build_http_client`.
fn build_throughput_client(
    hostname: &str,
    addr: SocketAddr,
    settings: &AppSettings,
) -> Result<Client> {
    let user_agent = settings.download.default_user_agent.trim();
    let user_agent = if user_agent.is_empty() {
        default_http_user_agent()
    } else {
        user_agent.to_string()
    };

    let builder = Client::builder()
        .resolve_to_addrs(hostname, &[addr])
        .connect_timeout(Duration::from_secs(5))
        .default_headers(reqwest::header::HeaderMap::from_iter([
            (
                reqwest::header::ACCEPT,
                reqwest::header::HeaderValue::from_static("*/*"),
            ),
            (
                reqwest::header::ACCEPT_LANGUAGE,
                reqwest::header::HeaderValue::from_static("en-US,en;q=0.9"),
            ),
        ]));

    let mut builder = configure_client_builder(builder, settings)?;
    // Override user_agent with our custom fallback logic
    builder = builder.user_agent(user_agent);

    builder.build().map_err(DownloadError::from)
}

/// Measure HTTPS download throughput to a candidate IP address.
///
/// Builds a throwaway `reqwest::Client` that resolves `hostname` to `ip`,
/// then streams `url` for up to `SPEED_TEST_DURATION`. Returns
/// `(bytes_downloaded, elapsed_ms)`. On timeout the partial bytes
/// accumulated so far are returned instead of an error.
pub async fn measure_throughput(
    ip: Ipv4Addr,
    hostname: &str,
    url: &str,
    settings: &AppSettings,
) -> Result<(f64, u64)> {
    tracing::info!("throughput test start: ip={ip} host={hostname}");

    let parsed = Url::parse(url)
        .map_err(|e| DownloadError::InvalidResponse(format!("invalid speed-test URL: {e}")))?;
    let port = parsed
        .port()
        .unwrap_or_else(|| if parsed.scheme() == "https" { 443 } else { 80 });

    let addr = SocketAddr::new(IpAddr::V4(ip), port);
    let client = build_throughput_client(hostname, addr, settings)?;

    let start = Instant::now();

    // Wrap send() in a timeout — it covers TCP connect + TLS handshake + request + response headers.
    // Without this, a host that passes TCP screening but hangs on TLS will block forever.
    let response = match timeout(Duration::from_secs(15), client.get(url).send()).await {
        Ok(Ok(resp)) => resp,
        Ok(Err(e)) => {
            tracing::warn!(
                "throughput test send failed: ip={ip} elapsed={}ms err={e}",
                start.elapsed().as_millis()
            );
            return Err(e.into());
        }
        Err(_) => {
            tracing::warn!(
                "throughput test send timed out: ip={ip} after 15s (connect+TLS+headers)"
            );
            return Err(DownloadError::InvalidResponse(
                "send timed out after 15s (connect/TLS/headers)".into(),
            ));
        }
    };

    let status = response.status();
    tracing::debug!(
        "throughput test response: ip={ip} status={status} elapsed={}ms",
        start.elapsed().as_millis()
    );

    if !status.is_success() {
        tracing::warn!(
            "throughput test rejected: ip={ip} status={status} (non-2xx, skipping body stream)"
        );
        return Err(DownloadError::InvalidResponse(format!(
            "HTTP {status} from speed test endpoint",
        )));
    }

    let bytes = Arc::new(AtomicU64::new(0));
    let bytes_ref = Arc::clone(&bytes);

    let download = async move {
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(data) => {
                    bytes_ref.fetch_add(data.len() as u64, Ordering::Relaxed);
                }
                Err(e) => {
                    tracing::warn!("throughput test stream error: ip={ip} err={e}");
                    break;
                }
            }
        }
    };

    let _ = timeout(SPEED_TEST_DURATION, download).await;

    let elapsed_ms = start.elapsed().as_millis() as u64;
    let total_bytes = bytes.load(Ordering::Relaxed) as f64;

    tracing::info!(
        "throughput test done: ip={ip} bytes={total_bytes:.0} elapsed={elapsed_ms}ms throughput={:.2} MB/s",
        if elapsed_ms > 0 {
            total_bytes / (elapsed_ms as f64 / 1000.0) / 1_000_000.0
        } else {
            0.0
        }
    );

    Ok((total_bytes, elapsed_ms))
}

/// Measure TCP connect latency to a single address.
///
/// Wraps `TcpStream::connect` with a timeout. Returns `Some(elapsed)` on
/// successful connect or `None` if the connection times out or is refused.
pub async fn measure_tcp_latency(addr: SocketAddr, connect_timeout: Duration) -> Option<Duration> {
    let start = Instant::now();
    match timeout(connect_timeout, TcpStream::connect(addr)).await {
        Ok(Ok(stream)) => {
            drop(stream);
            Some(start.elapsed())
        }
        _ => None,
    }
}

/// Screen a list of candidate IPv4 addresses by TCP connect latency.
///
/// Tests up to `concurrency` IPs simultaneously on port 443 (HTTPS).
/// Returns reachable IPs sorted by latency ascending (fastest first).
pub async fn screen_candidates(
    ips: &[Ipv4Addr],
    concurrency: usize,
    connect_timeout: Duration,
) -> Vec<(Ipv4Addr, Duration)> {
    tracing::info!(
        "screening start: {} IPs, concurrency={concurrency}",
        ips.len()
    );
    let mut join_set = JoinSet::new();
    let mut results = Vec::with_capacity(ips.len());
    let mut ip_iter = ips.iter().copied();

    for _ in 0..concurrency {
        let Some(ip) = ip_iter.next() else {
            break;
        };
        join_set.spawn(async move {
            let addr = SocketAddr::new(IpAddr::V4(ip), 443);
            (ip, measure_tcp_latency(addr, connect_timeout).await)
        });
    }

    while let Some(result) = join_set.join_next().await {
        if let Ok((ip, Some(latency))) = result {
            results.push((ip, latency));
        }
        if let Some(ip) = ip_iter.next() {
            join_set.spawn(async move {
                let addr = SocketAddr::new(IpAddr::V4(ip), 443);
                (ip, measure_tcp_latency(addr, connect_timeout).await)
            });
        }
    }

    results.sort_by_key(|a| a.1);

    tracing::info!(
        "screening done: {}/{} IPs reachable, top latency={}ms",
        results.len(),
        ips.len(),
        results.first().map(|d| d.1.as_millis()).unwrap_or(0),
    );

    results
}

/// Run the two-phase speed test orchestrator.
///
/// Phase 1 — TCP connect-latency screening of all `ips`.
/// Phase 2 — HTTPS throughput measurement for the top N candidates
/// (concurrent via `JoinSet`). Results are sorted by throughput
/// descending (None sorts last), tie-broken by TCP latency ascending.
///
/// If `progress` is provided, it is called at phase transitions
/// and as each candidate completes Phase 2 throughput testing.
pub async fn run_speed_test(
    ips: &[Ipv4Addr],
    config: &SpeedTestConfig,
    settings: &AppSettings,
    progress: Option<ProgressFn>,
) -> Vec<SpeedTestResult> {
    let total_ips = ips.len() as u64;

    tracing::info!("speed test orchestrator: {total_ips} candidate IPs");

    // ── Phase 1: TCP screening ─────────────────────────────────
    if let Some(ref p) = progress {
        p(CdnTestPhase::Screening, 0, total_ips);
    }

    let candidates = screen_candidates(ips, config.concurrency, config.tcp_timeout).await;

    if let Some(ref p) = progress {
        p(CdnTestPhase::Screening, total_ips, total_ips);
    }

    let top_n: Vec<(Ipv4Addr, Duration)> = candidates
        .into_iter()
        .take(config.top_n_candidates)
        .collect();

    if top_n.is_empty() {
        tracing::warn!("speed test: no reachable IPs after screening, aborting");
        return Vec::new();
    }

    let top_count = top_n.len() as u64;

    tracing::info!(
        "speed test: top {top_count} IPs advancing to throughput testing: {}",
        top_n
            .iter()
            .map(|(ip, d)| format!("{ip}({}ms", d.as_millis()))
            .collect::<Vec<_>>()
            .join(", "),
    );

    // ── Phase 2: Concurrent throughput testing ─────────────────
    if let Some(ref p) = progress {
        p(CdnTestPhase::MeasuringThroughput, 0, top_count);
    }

    let mut join_set = JoinSet::new();
    for (ip, latency) in &top_n {
        let ip = *ip;
        let latency = *latency;
        let s = settings.clone();
        join_set.spawn(async move {
            let result = measure_throughput(ip, "speed.cloudflare.com", SPEED_TEST_URL, &s).await;
            (ip, latency, result)
        });
    }

    let mut results = Vec::with_capacity(top_n.len());
    let mut completed: u64 = 0;

    while let Some(task_result) = join_set.join_next().await {
        completed += 1;
        match task_result {
            Ok((ip, latency, throughput_result)) => {
                let tcp_latency_ms = latency.as_secs_f64() * 1000.0;
                match throughput_result {
                    Ok((bytes, elapsed_ms)) => {
                        let elapsed_secs = elapsed_ms as f64 / 1000.0;
                        let throughput_mbps = if elapsed_secs > 0.0 {
                            Some((bytes / elapsed_secs) / 1_000_000.0)
                        } else {
                            None
                        };
                        tracing::info!(
                            "throughput candidate {completed}/{top_count}: ip={ip} {}bytes {elapsed_ms}ms {:.2}MB/s",
                            if bytes > 1_000_000.0 {
                                format!("{:.1}MB ", bytes / 1_000_000.0)
                            } else {
                                format!("{}B ", bytes as u64)
                            },
                            throughput_mbps.unwrap_or(0.0),
                        );
                        results.push(SpeedTestResult {
                            ip,
                            tcp_latency_ms,
                            throughput_mbps,
                            error: None,
                        });
                    }
                    Err(e) => {
                        tracing::warn!(
                            "throughput candidate {completed}/{top_count}: ip={ip} FAILED: {e}",
                        );
                        results.push(SpeedTestResult {
                            ip,
                            tcp_latency_ms,
                            throughput_mbps: None,
                            error: Some(e.to_string()),
                        });
                    }
                }
            }
            Err(join_err) => {
                tracing::error!("throughput task panicked (JoinError): {join_err}");
            }
        }
        if let Some(ref p) = progress {
            p(CdnTestPhase::MeasuringThroughput, completed, top_count);
        }
    }

    tracing::info!(
        "speed test orchestrator done: {}/{} throughput tests completed, {} with valid throughput",
        results.len(),
        top_count,
        results
            .iter()
            .filter(|r| r.throughput_mbps.is_some())
            .count(),
    );

    results.sort_by(|a, b| {
        b.throughput_mbps
            .partial_cmp(&a.throughput_mbps)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                a.tcp_latency_ms
                    .partial_cmp(&b.tcp_latency_ms)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn test_measure_latency_to_localhost() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let latency = measure_tcp_latency(addr, Duration::from_secs(5)).await;
        assert!(latency.is_some(), "localhost connection should succeed");
        assert!(
            latency.unwrap() < Duration::from_millis(100),
            "localhost latency should be under 100ms"
        );

        drop(listener);
    }

    #[tokio::test]
    async fn test_measure_latency_unreachable() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1);
        let latency = measure_tcp_latency(addr, Duration::from_secs(5)).await;
        assert!(latency.is_none(), "closed port should be unreachable");
    }

    #[tokio::test]
    async fn test_screen_candidates_concurrent() {
        // Class-E reserved (240.0.0.0/4) — unroutable on any normal network.
        let ips: Vec<Ipv4Addr> = (0..10).map(|i| Ipv4Addr::new(240, 0, 0, i + 1)).collect();

        let results = screen_candidates(&ips, 5, Duration::from_secs(2)).await;
        assert!(
            results.is_empty(),
            "class-E IPs should be unreachable, got {} results",
            results.len()
        );
    }

    #[tokio::test]
    async fn test_throughput_to_localhost() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let body = vec![b'X'; 1024 * 1024];
            let headers = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(headers.as_bytes()).await.unwrap();
            stream.write_all(&body).await.unwrap();
            stream.flush().await.unwrap();
            // Keep connection alive until client disconnects
            let mut buf = [0u8; 1];
            let _ = stream.read(&mut buf).await;
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        let url = format!("http://127.0.0.1:{}/test", port);
        let settings = AppSettings::default();

        let result = measure_throughput(Ipv4Addr::LOCALHOST, "127.0.0.1", &url, &settings).await;

        assert!(
            result.is_ok(),
            "throughput to localhost should succeed, got: {result:?}"
        );
        let (bytes, _elapsed) = result.unwrap();
        assert!(bytes > 0.0, "should have downloaded some bytes");
    }

    #[tokio::test]
    #[ignore = "network-dependent: TEST-NET-1 (192.0.2.0/24) may be intercepted by proxies/VPNs in some environments"]
    async fn test_throughput_unreachable() {
        // 192.0.2.0/24 is TEST-NET-1 — RFC 5737 reserved, never routable
        let unreachable = Ipv4Addr::new(192, 0, 2, 1);
        let settings = AppSettings::default();

        let result = measure_throughput(
            unreachable,
            "speed.cloudflare.com",
            SPEED_TEST_URL,
            &settings,
        )
        .await;

        match result {
            Err(_) => {}
            Ok((bytes, elapsed)) => {
                assert!(
                    bytes < 1024.0 || elapsed >= 9000,
                    "unreachable IP should yield error or negligible data, got {bytes} bytes in {elapsed}ms"
                );
            }
        }
    }

    // ── Orchestrator tests ────────────────────────────────────

    #[tokio::test]
    async fn test_orchestrator_all_unreachable() {
        // Class-E reserved (240.0.0.0/4) — unroutable on any normal network.
        let ips: Vec<Ipv4Addr> = (1..=5).map(|i| Ipv4Addr::new(240, 0, 0, i)).collect();
        let config = SpeedTestConfig::default();
        let settings = AppSettings::default();

        let results = run_speed_test(&ips, &config, &settings, None).await;
        assert!(
            results.is_empty(),
            "all class-E IPs unreachable → empty results, got {}",
            results.len()
        );
    }

    #[tokio::test]
    async fn test_orchestrator_with_mock_ips() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        // Bind on localhost:443 so Phase 1 TCP screening passes.
        let listener = match tokio::net::TcpListener::bind("127.0.0.1:443").await {
            Ok(l) => l,
            Err(_) => {
                eprintln!("SKIP: cannot bind port 443");
                return;
            }
        };

        tokio::spawn(async move {
            loop {
                let (mut stream, _) = match listener.accept().await {
                    Ok(c) => c,
                    Err(_) => break,
                };
                tokio::spawn(async move {
                    let mut buf = [0u8; 4];
                    let n = stream.read(&mut buf).await.unwrap_or(0);
                    // Serve plain HTTP if it's an HTTP request (Phase 2
                    // will fail TLS anyway, but we keep the handler for
                    // completeness).
                    if n > 0 && &buf[..n] == b"GET " {
                        let body = vec![b'X'; 512 * 1024];
                        let headers = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(headers.as_bytes()).await;
                        let _ = stream.write_all(&body).await;
                        let _ = stream.flush().await;
                        let _ = stream.read(&mut buf).await;
                    }
                });
            }
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        let ips = vec![
            Ipv4Addr::LOCALHOST,
            Ipv4Addr::LOCALHOST,
            Ipv4Addr::LOCALHOST,
        ];
        let config = SpeedTestConfig {
            top_n_candidates: 3,
            ..SpeedTestConfig::default()
        };
        let settings = AppSettings::default();

        let results = run_speed_test(&ips, &config, &settings, None).await;

        assert!(
            !results.is_empty(),
            "Phase 1 should pass with listener on :443"
        );

        for r in &results {
            assert_eq!(r.ip, Ipv4Addr::LOCALHOST);
            assert!(r.tcp_latency_ms >= 0.0);
            // Phase 2 fails because our server is plain TCP, not TLS.
            assert!(
                r.error.is_some(),
                "Phase 2 should fail (TLS), got throughput={:?}",
                r.throughput_mbps
            );
        }

        // All throughputs are None → sorted by latency ascending.
        for i in 1..results.len() {
            assert!(
                results[i - 1].tcp_latency_ms <= results[i].tcp_latency_ms,
                "tiebreak sort: latency ascending"
            );
        }
    }

    #[tokio::test]
    async fn test_orchestrator_partial_failures() {
        // Mix class-E (unreachable) + localhost (reachable if :443 open).
        let ips: Vec<Ipv4Addr> = [
            Ipv4Addr::new(240, 0, 0, 1),
            Ipv4Addr::new(240, 0, 0, 2),
            Ipv4Addr::LOCALHOST,
            Ipv4Addr::new(240, 0, 0, 3),
        ]
        .to_vec();

        let config = SpeedTestConfig::default();
        let settings = AppSettings::default();

        let results = run_speed_test(&ips, &config, &settings, None).await;

        // Class-E IPs must never appear — they fail Phase 1.
        for r in &results {
            let octets = r.ip.octets();
            assert!(
                octets[0] != 240,
                "class-E IP {:?} must not appear in results",
                r.ip
            );
        }

        // If localhost was reachable (port 443 open), results are non-empty.
        if !results.is_empty() {
            for r in &results {
                assert!(r.tcp_latency_ms >= 0.0);
            }
        }
    }
}
