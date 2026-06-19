use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use reqwest::redirect::Policy;
use reqwest::{Client, Proxy, Url};
use tokio::net::TcpStream;
use tokio::task::JoinSet;
use tokio::time::timeout;

use crate::download::error::{DownloadError, Result};
use crate::download::types::{
    default_http_user_agent, AppSettings, ProxyMode,
};

/// Cloudflare CDN speed test endpoint (200MB file).
pub(crate) const SPEED_TEST_URL: &str =
    "https://speed.cloudflare.com/__down?bytes=200000000";

/// Maximum duration for a single IP's throughput test.
pub(crate) const SPEED_TEST_DURATION: Duration = Duration::from_secs(10);

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

    let mut builder = Client::builder()
        .resolve_to_addrs(hostname, &[addr])
        .tcp_nodelay(true)
        .read_timeout(Duration::from_secs(15))
        .user_agent(user_agent)
        .redirect(Policy::limited(10));

    match settings.proxy.mode {
        ProxyMode::Disabled => {
            builder = builder.no_proxy();
        }
        ProxyMode::System => {}
        ProxyMode::Manual => {
            let proxy = Proxy::all(&settings.proxy.manual_url)
                .map_err(|e| DownloadError::InvalidProxy(e.to_string()))?;
            builder = builder.proxy(proxy);
        }
    }

    builder.build().map_err(DownloadError::from)
}

/// Measure HTTPS download throughput to a candidate IP address.
///
/// Builds a throwaway `reqwest::Client` that resolves `hostname` to `ip`,
/// then streams `url` for up to `SPEED_TEST_DURATION`. Returns
/// `(bytes_downloaded, elapsed_ms)`. On timeout the partial bytes
/// accumulated so far are returned instead of an error.
pub(crate) async fn measure_throughput(
    ip: Ipv4Addr,
    hostname: &str,
    url: &str,
    settings: &AppSettings,
) -> Result<(f64, u64)> {
    let parsed = Url::parse(url).map_err(|e| {
        DownloadError::InvalidResponse(format!("invalid speed-test URL: {e}"))
    })?;
    let port = parsed
        .port()
        .unwrap_or_else(|| if parsed.scheme() == "https" { 443 } else { 80 });

    let addr = SocketAddr::new(IpAddr::V4(ip), port);
    let client = build_throughput_client(hostname, addr, settings)?;

    let start = Instant::now();
    let response = client.get(url).send().await?;

    let bytes = Arc::new(AtomicU64::new(0));
    let bytes_ref = Arc::clone(&bytes);

    let download = async move {
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(data) => {
                    bytes_ref.fetch_add(data.len() as u64, Ordering::Relaxed);
                }
                Err(_) => break,
            }
        }
    };

    let _ = timeout(SPEED_TEST_DURATION, download).await;

    let elapsed_ms = start.elapsed().as_millis() as u64;
    let total_bytes = bytes.load(Ordering::Relaxed) as f64;

    Ok((total_bytes, elapsed_ms))
}

/// Measure TCP connect latency to a single address.
///
/// Wraps `TcpStream::connect` with a timeout. Returns `Some(elapsed)` on
/// successful connect or `None` if the connection times out or is refused.
pub(crate) async fn measure_tcp_latency(
    addr: SocketAddr,
    connect_timeout: Duration,
) -> Option<Duration> {
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
pub(crate) async fn screen_candidates(
    ips: &[Ipv4Addr],
    concurrency: usize,
    connect_timeout: Duration,
) -> Vec<(Ipv4Addr, Duration)> {
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

    results.sort_by(|a, b| a.1.cmp(&b.1));
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
        let ips: Vec<Ipv4Addr> = (0..10).map(|_| Ipv4Addr::LOCALHOST).collect();

        let results = screen_candidates(&ips, 5, Duration::from_secs(5)).await;
        assert!(
            results.is_empty(),
            "closed port should be unreachable, got {} results",
            results.len()
        );
    }

    #[tokio::test]
    async fn test_throughput_to_localhost() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap();
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

        let result = measure_throughput(
            Ipv4Addr::LOCALHOST,
            "127.0.0.1",
            &url,
            &settings,
        )
        .await;

        assert!(
            result.is_ok(),
            "throughput to localhost should succeed, got: {result:?}"
        );
        let (bytes, _elapsed) = result.unwrap();
        assert!(bytes > 0.0, "should have downloaded some bytes");
    }

    #[tokio::test]
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
}
