use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

use tokio::net::TcpStream;
use tokio::task::JoinSet;
use tokio::time::timeout;

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
}
