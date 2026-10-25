use super::*;
use ntest::timeout;

type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test]
#[timeout(30_000)]
async fn downloads_full_file() -> TestResult {
    let server = TestServer::new(64 * 1024).await;
    let body = reqwest::get(server.file_url()).await?.bytes().await?;
    assert_eq!(body.len() as u64, server.file_size);
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn checksums_match_computed_content() -> TestResult {
    let server = TestServer::new(64 * 1024).await;
    let body = reqwest::get(server.file_url()).await?.bytes().await?;

    let slices = &[&body[..]];
    assert_eq!(
        hash_slices(ChecksumMode::Blake3, slices),
        server.blake3_hash,
        "Blake3 checksum mismatch",
    );
    assert_eq!(
        hash_slices(ChecksumMode::Sha256, slices),
        server.sha256_hash,
        "SHA-256 checksum mismatch",
    );
    assert_eq!(
        hash_slices(ChecksumMode::Sha512, slices),
        server.sha512_hash,
        "SHA-512 checksum mismatch",
    );
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn range_request_returns_correct_bytes() -> TestResult {
    let server = TestServer::new(64 * 1024).await;
    let range_url = format!("{}/file/range", server.addr);

    let client = reqwest::Client::new();
    let response = client
        .get(&range_url)
        .header(header::RANGE, "bytes=1000-1999")
        .send()
        .await?;

    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    let body = response.bytes().await?;
    assert_eq!(body.len(), 1000);

    // Must match the deterministic random content at that offset
    let expected = generate_content(64 * 1024);
    assert_eq!(&body[..], &expected[1000..2000]);
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn range_bandwidth_endpoint_serves_throttled_ranges() -> TestResult {
    let server = TestServer::new(64 * 1024).await;
    // 2 MiB/s → 64 KB chunks take ~32 ms, so a 1000-byte range still waits one tick.
    let url = server.file_url_range_bandwidth(2 * 1024 * 1024);

    let client = reqwest::Client::new();
    let start = std::time::Instant::now();
    let response = client
        .get(&url)
        .header(header::RANGE, "bytes=1000-1999")
        .send()
        .await?;
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert!(response.headers().contains_key(header::ACCEPT_RANGES));
    let body = response.bytes().await?;
    assert!(
        start.elapsed() >= Duration::from_millis(20),
        "range-bandwidth endpoint did not throttle"
    );
    assert_eq!(body.len(), 1000);

    let expected = generate_content(64 * 1024);
    assert_eq!(&body[..], &expected[1000..2000]);
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn range_bandwidth_endpoint_serves_full_file_without_range() -> TestResult {
    let server = TestServer::new(64 * 1024).await;
    // bps = 0 disables the per-chunk delay; this exercises the no-Range path.
    let body = reqwest::get(server.file_url_range_bandwidth(0))
        .await?
        .bytes()
        .await?;
    assert_eq!(body.len() as u64, server.file_size);

    let expected = generate_content(64 * 1024);
    assert_eq!(&body[..], &expected);
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn delayed_endpoint_content_matches() -> TestResult {
    let server = TestServer::new(16 * 1024).await;
    let delayed_url = format!("{}/file/delayed/250", server.addr);

    let start = std::time::Instant::now();
    let body = reqwest::get(&delayed_url).await?.bytes().await?;
    let elapsed = start.elapsed();

    assert_eq!(body.len() as u64, server.file_size);
    assert!(
        elapsed >= Duration::from_millis(250),
        "expected at least 250ms delay but got {elapsed:?}",
    );

    // Content must still be identical
    let expected = generate_content(16 * 1024);
    assert_eq!(&body[..], &expected);
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn truncated_endpoint_serves_partial_content() -> TestResult {
    let server = TestServer::new(64 * 1024).await;
    let truncated_url = format!("{}/file/truncated/512", server.addr);

    let body = reqwest::get(&truncated_url).await?.bytes().await?;
    assert_eq!(body.len(), 512);

    // Content must be the first 512 bytes of the deterministic file
    let expected = generate_content(64 * 1024);
    assert_eq!(&body[..], &expected[..512]);
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn range_shifted_endpoint_serves_shifted_content() -> TestResult {
    let server = TestServer::new(64 * 1024).await;
    let url = server.file_url_range_shifted(1);

    let client = reqwest::Client::new();
    // Advertised range 1000-1999, but content is data[1001..=2000].
    let response = client
        .get(&url)
        .header(header::RANGE, "bytes=1000-1999")
        .send()
        .await?;
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    let body = response.bytes().await?;
    assert_eq!(body.len(), 1000);

    let expected = generate_content(64 * 1024);
    assert_eq!(
        &body[..],
        &expected[1001..2001],
        "range-shifted server returned wrong bytes"
    );
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn range_bitflip_endpoint_corrupts_content() -> TestResult {
    let server = TestServer::new(64 * 1024).await;
    let url = server.file_url_range_bitflip();

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header(header::RANGE, "bytes=0-31")
        .send()
        .await?;
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    let body = response.bytes().await?;
    assert_eq!(body.len(), 32);

    let expected = generate_content(64 * 1024);
    assert_ne!(
        &body[..],
        &expected[..32],
        "bitflip server must corrupt the range"
    );
    // Only the first byte differs; the rest is intact.
    assert_eq!(&body[1..], &expected[1..32]);
    Ok(())
}
