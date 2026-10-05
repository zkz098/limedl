use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use super::scorer::{MirrorScoringConfig, calculate_mirror_score};
use super::types::MirrorResource;

/// Thread-safe pool managing multiple mirror resources and their connection leases.
#[derive(Debug, Clone)]
pub struct MirrorPool {
    inner: Arc<Mutex<MirrorPoolInner>>,
}

#[derive(Debug)]
struct MirrorPoolInner {
    mirrors: Vec<MirrorCandidate>,
    config: MirrorScoringConfig,
    _default_max_connections_per_server: usize,
}

/// A mirror candidate with active runtime metrics and health status.
#[derive(Debug, Clone)]
pub struct MirrorCandidate {
    pub url: String,
    pub priority: u32,
    pub location: Option<String>,
    pub max_connections: usize,
    pub active_connections: usize,
    pub score: f64,
    pub consecutive_errors: u32,
    pub backoff_until: Option<Instant>,
    pub total_bytes_downloaded: u64,
    pub chunks_completed: usize,
    pub rtt_ms: Option<u64>,
}

/// Live snapshot of a mirror's status for UI/RPC display.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MirrorStatusSnapshot {
    pub url: String,
    pub location: Option<String>,
    pub priority: u32,
    pub active_connections: usize,
    pub max_connections: usize,
    pub score: f64,
    pub rtt_ms: Option<u64>,
    pub chunks_completed: usize,
    pub total_bytes: u64,
    pub is_cooling_down: bool,
}

impl MirrorPool {
    /// Create a new mirror pool from mirror resources and configuration.
    pub fn new(
        resources: Vec<MirrorResource>,
        config: MirrorScoringConfig,
        default_max_connections_per_server: usize,
    ) -> Self {
        let mirrors = resources
            .into_iter()
            .map(|r| {
                let score = calculate_mirror_score(&r, None, 0, &config);
                MirrorCandidate {
                    url: r.url,
                    priority: r.priority,
                    location: r.location,
                    max_connections: r
                        .max_connections
                        .unwrap_or(default_max_connections_per_server)
                        .max(1),
                    active_connections: 0,
                    score,
                    consecutive_errors: 0,
                    backoff_until: None,
                    total_bytes_downloaded: 0,
                    chunks_completed: 0,
                    rtt_ms: None,
                }
            })
            .collect();

        Self {
            inner: Arc::new(Mutex::new(MirrorPoolInner {
                mirrors,
                config,
                _default_max_connections_per_server: default_max_connections_per_server,
            })),
        }
    }

    /// Construct a pool from a single primary URL and optional mirror URLs.
    pub fn from_urls(
        primary_url: &str,
        mirror_urls: &[String],
        config: MirrorScoringConfig,
        max_conn_per_server: usize,
    ) -> Self {
        let mut resources = Vec::new();

        // Primary URL has highest priority (1)
        resources.push(MirrorResource {
            url: primary_url.to_string(),
            priority: 1,
            location: None,
            max_connections: Some(max_conn_per_server),
        });

        for (idx, m) in mirror_urls.iter().enumerate() {
            if m != primary_url {
                resources.push(MirrorResource {
                    url: m.clone(),
                    priority: (idx as u32) + 2,
                    location: None,
                    max_connections: Some(max_conn_per_server),
                });
            }
        }

        Self::new(resources, config, max_conn_per_server)
    }

    /// Update RTT latency metrics from pre-flight probe results.
    pub fn update_rtt(&self, url: &str, rtt_ms: u64) {
        let mut inner = self.inner.lock();
        let config = inner.config.clone();
        if let Some(m) = inner.mirrors.iter_mut().find(|m| m.url == url) {
            m.rtt_ms = Some(rtt_ms);
            let resource = MirrorResource {
                url: m.url.clone(),
                priority: m.priority,
                location: m.location.clone(),
                max_connections: Some(m.max_connections),
            };
            m.score = calculate_mirror_score(&resource, Some(rtt_ms), m.consecutive_errors, &config);
        }
    }

    /// Lease the highest-scoring available mirror that has not reached its max concurrency limit.
    pub fn lease_best_mirror(&self) -> Option<MirrorLease> {
        let mut inner = self.inner.lock();
        let now = Instant::now();

        // Filter mirrors that are not in cooldown and have remaining connection slots
        let mut best_idx = None;
        let mut best_score = f64::NEG_INFINITY;

        for (idx, m) in inner.mirrors.iter().enumerate() {
            if let Some(backoff) = m.backoff_until
                && backoff > now
            {
                continue; // In cooldown
            }
            if m.active_connections >= m.max_connections {
                continue; // Capacity reached
            }
            if m.score > best_score {
                best_score = m.score;
                best_idx = Some(idx);
            }
        }

        // Fallback: if all eligible are at capacity, pick the one with fewest active connections
        // as long as it's not cooling down
        if best_idx.is_none() {
            let mut least_conns = usize::MAX;
            for (idx, m) in inner.mirrors.iter().enumerate() {
                if let Some(backoff) = m.backoff_until
                    && backoff > now
                {
                    continue;
                }
                if m.active_connections < least_conns {
                    least_conns = m.active_connections;
                    best_idx = Some(idx);
                }
            }
        }

        let target_idx = best_idx?;
        let mirror = &mut inner.mirrors[target_idx];
        mirror.active_connections = mirror.active_connections.saturating_add(1);

        Some(MirrorLease {
            pool: self.clone(),
            url: mirror.url.clone(),
        })
    }

    /// Report that a chunk download succeeded on the given mirror.
    pub fn report_success(&self, url: &str, bytes: u64) {
        let mut inner = self.inner.lock();
        let config = inner.config.clone();
        if let Some(m) = inner.mirrors.iter_mut().find(|m| m.url == url) {
            m.consecutive_errors = 0;
            m.backoff_until = None;
            m.total_bytes_downloaded = m.total_bytes_downloaded.saturating_add(bytes);
            m.chunks_completed = m.chunks_completed.saturating_add(1);
            let resource = MirrorResource {
                url: m.url.clone(),
                priority: m.priority,
                location: m.location.clone(),
                max_connections: Some(m.max_connections),
            };
            m.score = calculate_mirror_score(&resource, m.rtt_ms, 0, &config);
        }
    }

    /// Report a network/HTTP error on the given mirror, applying exponential backoff.
    pub fn report_failure(&self, url: &str) {
        let mut inner = self.inner.lock();
        let config = inner.config.clone();
        if let Some(m) = inner.mirrors.iter_mut().find(|m| m.url == url) {
            m.consecutive_errors = m.consecutive_errors.saturating_add(1);
            let backoff_secs = (2u64).pow(m.consecutive_errors.min(6)).min(60);
            m.backoff_until = Some(Instant::now() + Duration::from_secs(backoff_secs));

            let resource = MirrorResource {
                url: m.url.clone(),
                priority: m.priority,
                location: m.location.clone(),
                max_connections: Some(m.max_connections),
            };
            m.score = calculate_mirror_score(&resource, m.rtt_ms, m.consecutive_errors, &config);
        }
    }

    /// Severely penalize a mirror that provided a corrupted chunk/piece.
    pub fn report_corrupted_piece(&self, url: &str) {
        let mut inner = self.inner.lock();
        let config = inner.config.clone();
        if let Some(m) = inner.mirrors.iter_mut().find(|m| m.url == url) {
            m.consecutive_errors = m.consecutive_errors.saturating_add(5);
            // 5-minute isolation penalty for hash corruption
            m.backoff_until = Some(Instant::now() + Duration::from_secs(300));

            let resource = MirrorResource {
                url: m.url.clone(),
                priority: m.priority,
                location: m.location.clone(),
                max_connections: Some(m.max_connections),
            };
            m.score = calculate_mirror_score(&resource, m.rtt_ms, m.consecutive_errors, &config);
        }
    }

    fn release_connection(&self, url: &str) {
        let mut inner = self.inner.lock();
        if let Some(m) = inner.mirrors.iter_mut().find(|m| m.url == url) {
            m.active_connections = m.active_connections.saturating_sub(1);
        }
    }

    /// Get live status snapshots of all mirrors.
    pub fn snapshots(&self) -> Vec<MirrorStatusSnapshot> {
        let inner = self.inner.lock();
        let now = Instant::now();
        inner
            .mirrors
            .iter()
            .map(|m| MirrorStatusSnapshot {
                url: m.url.clone(),
                location: m.location.clone(),
                priority: m.priority,
                active_connections: m.active_connections,
                max_connections: m.max_connections,
                score: m.score,
                rtt_ms: m.rtt_ms,
                chunks_completed: m.chunks_completed,
                total_bytes: m.total_bytes_downloaded,
                is_cooling_down: m.backoff_until.is_some_and(|t| t > now),
            })
            .collect()
    }
}

/// RAII guard representing a leased connection slot to a specific mirror.
/// Decrements the mirror's active connection count when dropped.
#[derive(Debug)]
pub struct MirrorLease {
    pool: MirrorPool,
    pub url: String,
}

impl MirrorLease {
    pub fn report_success(&self, bytes: u64) {
        self.pool.report_success(&self.url, bytes);
    }

    pub fn report_failure(&self) {
        self.pool.report_failure(&self.url);
    }

    pub fn report_corrupted_piece(&self) {
        self.pool.report_corrupted_piece(&self.url);
    }
}

impl Drop for MirrorLease {
    fn drop(&mut self) {
        self.pool.release_connection(&self.url);
    }
}
