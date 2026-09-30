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
    let ips: Vec<IpAddr> = (0..10)
        .map(|i| IpAddr::V4(Ipv4Addr::new(240, 0, 0, i + 1)))
        .collect();

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

    let result = measure_throughput(
        IpAddr::V4(Ipv4Addr::LOCALHOST),
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
#[ignore = "network-dependent: TEST-NET-1 (192.0.2.0/24) may be intercepted by proxies/VPNs in some environments"]
async fn test_throughput_unreachable() {
    // 192.0.2.0/24 is TEST-NET-1 — RFC 5737 reserved, never routable
    let unreachable = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1));
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
    let ips: Vec<IpAddr> = (1..=5)
        .map(|i| IpAddr::V4(Ipv4Addr::new(240, 0, 0, i)))
        .collect();
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
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V4(Ipv4Addr::LOCALHOST),
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
        assert_eq!(r.ip, IpAddr::V4(Ipv4Addr::LOCALHOST));
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
    let ips: Vec<IpAddr> = [
        IpAddr::V4(Ipv4Addr::new(240, 0, 0, 1)),
        IpAddr::V4(Ipv4Addr::new(240, 0, 0, 2)),
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V4(Ipv4Addr::new(240, 0, 0, 3)),
    ]
    .to_vec();

    let config = SpeedTestConfig::default();
    let settings = AppSettings::default();

    let results = run_speed_test(&ips, &config, &settings, None).await;

    // Class-E IPs must never appear — they fail Phase 1.
    for r in &results {
        if let IpAddr::V4(v4) = r.ip {
            assert!(
                v4.octets()[0] != 240,
                "class-E IP {:?} must not appear in results",
                r.ip
            );
        }
    }

    // If localhost was reachable (port 443 open), results are non-empty.
    if !results.is_empty() {
        for r in &results {
            assert!(r.tcp_latency_ms >= 0.0);
        }
    }
}
