use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::stream::{FuturesUnordered, StreamExt};
use reqwest::header::{ACCEPT_RANGES, HeaderValue, RANGE};
use reqwest::{Client, StatusCode};

use super::types::MirrorResource;

/// Result of probing a single mirror.
#[derive(Debug, Clone)]
pub struct ProbeResult {
    pub url: String,
    pub reachable: bool,
    pub rtt_ms: Option<u64>,
    pub supports_ranges: bool,
    pub error: Option<String>,
}

/// Concurrently probe candidate mirrors with a short timeout.
pub async fn probe_mirrors(
    client: &Client,
    mirrors: &[MirrorResource],
    concurrency_limit: usize,
    probe_timeout: Duration,
) -> Vec<ProbeResult> {
    let client = Arc::new(client.clone());
    let mut futures = FuturesUnordered::new();
    let mut results = Vec::with_capacity(mirrors.len());

    let mut iter = mirrors.iter();

    // Fill initial batch up to concurrency limit
    for _ in 0..concurrency_limit {
        if let Some(mirror) = iter.next() {
            let client = client.clone();
            let url = mirror.url.clone();
            futures.push(tokio::spawn(async move {
                probe_single_mirror(&client, &url, probe_timeout).await
            }));
        }
    }

    // Process completed and refill
    while let Some(join_res) = futures.next().await {
        if let Ok(probe_res) = join_res {
            results.push(probe_res);
        }
        if let Some(mirror) = iter.next() {
            let client = client.clone();
            let url = mirror.url.clone();
            futures.push(tokio::spawn(async move {
                probe_single_mirror(&client, &url, probe_timeout).await
            }));
        }
    }

    results
}

async fn probe_single_mirror(client: &Client, url: &str, probe_timeout: Duration) -> ProbeResult {
    let start = Instant::now();

    // Use Range: bytes=0-0 to test both connectivity and range request support in a single round-trip
    let request_res = client
        .get(url)
        .header(RANGE, HeaderValue::from_static("bytes=0-0"))
        .timeout(probe_timeout)
        .send()
        .await;

    match request_res {
        Ok(resp) => {
            let rtt_ms = start.elapsed().as_millis() as u64;
            let status = resp.status();
            let reachable = status.is_success() || status == StatusCode::PARTIAL_CONTENT;

            let supports_ranges = status == StatusCode::PARTIAL_CONTENT
                || resp
                    .headers()
                    .get(ACCEPT_RANGES)
                    .and_then(|v| v.to_str().ok())
                    .is_some_and(|v| v.eq_ignore_ascii_case("bytes"));

            ProbeResult {
                url: url.to_string(),
                reachable,
                rtt_ms: if reachable { Some(rtt_ms) } else { None },
                supports_ranges,
                error: if reachable {
                    None
                } else {
                    Some(format!("HTTP status {status}"))
                },
            }
        }
        Err(err) => ProbeResult {
            url: url.to_string(),
            reachable: false,
            rtt_ms: None,
            supports_ranges: false,
            error: Some(err.to_string()),
        },
    }
}
