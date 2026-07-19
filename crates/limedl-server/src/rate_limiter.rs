use std::sync::Arc;

use dashmap::DashMap;
use governor::clock::DefaultClock;
use governor::state::direct::NotKeyed;
use governor::state::InMemoryState;
use governor::{Quota, RateLimiter};
use nonzero_ext::nonzero;

/// Classification of JSON-RPC method safety for rate limiting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodClass {
    /// Read-only / query methods: higher limits, lower penalty.
    Safe,
    /// Mutating / write methods: stricter limits.
    Mutating,
}

impl MethodClass {
    /// Classify a JSON-RPC method name into safe or mutating.
    pub fn classify(method: &str) -> Self {
        match method {
            // Safe (read-only) methods
            "download.list"
            | "download.status"
            | "settings.get"
            | "bt.runtimeStatus"
            | "bt.getPeers"
            | "bt.getTrackers"
            | "bt.getPieces"
            | "bt.getFiles"
            | "cdn.status"
            | "cdn.detail"
            | "cdn.fetchRanges"
            | "cdn.candidates"
            | "settings.getIoStatus"
            | "settings.getOverclockMode" => MethodClass::Safe,
            // Everything else is mutating
            _ => MethodClass::Mutating,
        }
    }
}

/// Per-connection rate limits for a single WebSocket connection.
struct ConnLimits {
    safe: RateLimiter<NotKeyed, InMemoryState, DefaultClock>,
    mutating: RateLimiter<NotKeyed, InMemoryState, DefaultClock>,
}

/// WebSocket JSON-RPC rate limiter.
///
/// Maintains per-connection RateLimiter instances that are created on
/// connection registration and dropped on unregistration. Global limits
/// are shared across all connections.
pub struct WsRateLimiter {
    /// Per-connection limiters stored by connection ID
    connections: DashMap<String, Arc<ConnLimits>>,
    /// Global limit for safe methods: 500/s, burst 750
    global_safe: RateLimiter<NotKeyed, InMemoryState, DefaultClock>,
    /// Global limit for mutating methods: 50/s, burst 100
    global_mutating: RateLimiter<NotKeyed, InMemoryState, DefaultClock>,
}

impl WsRateLimiter {
    /// Create a new `WsRateLimiter` with default per-connection and global limits.
    pub fn new() -> Self {
        Self {
            connections: DashMap::new(),
            global_safe: RateLimiter::direct(
                Quota::per_second(nonzero!(500u32))
                    .allow_burst(nonzero!(750u32)),
            ),
            global_mutating: RateLimiter::direct(
                Quota::per_second(nonzero!(50u32))
                    .allow_burst(nonzero!(100u32)),
            ),
        }
    }

    /// Check whether a request from `connection_id` of `class` is allowed.
    ///
    /// Returns `Ok(())` if within limits, or `Err` with a reason message
    /// if throttled.
    pub fn check(
        &self,
        connection_id: &str,
        class: MethodClass,
    ) -> Result<(), &'static str> {
        // Look up per-connection limiters
        let conn = self
            .connections
            .get(connection_id)
            .ok_or("connection not registered")?;

        match class {
            MethodClass::Safe => {
                conn.safe
                    .check()
                    .map_err(|_| "per-connection rate limit exceeded (safe)")?;
                self.global_safe
                    .check()
                    .map_err(|_| "global rate limit exceeded (safe)")?;
            }
            MethodClass::Mutating => {
                conn.mutating
                    .check()
                    .map_err(|_| "per-connection rate limit exceeded (mutating)")?;
                self.global_mutating
                    .check()
                    .map_err(|_| "global rate limit exceeded (mutating)")?;
            }
        }
        Ok(())
    }

    /// Register a new connection, creating per-connection rate limiters.
    pub fn register(&self, id: &str) {
        let limits = Arc::new(ConnLimits {
            safe: RateLimiter::direct(
                Quota::per_second(nonzero!(100u32))
                    .allow_burst(nonzero!(150u32)),
            ),
            mutating: RateLimiter::direct(
                Quota::per_second(nonzero!(10u32))
                    .allow_burst(nonzero!(20u32)),
            ),
        });
        self.connections.insert(id.to_owned(), limits);
    }

    /// Unregister a connection, dropping its per-connection rate limiters.
    pub fn unregister(&self, id: &str) {
        self.connections.remove(id);
    }
}

impl Default for WsRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_safe_methods() {
        for method in &[
            "download.list",
            "download.status",
            "settings.get",
            "bt.runtimeStatus",
            "bt.getPeers",
            "bt.getTrackers",
            "bt.getPieces",
            "bt.getFiles",
            "cdn.status",
            "cdn.detail",
            "cdn.fetchRanges",
            "cdn.candidates",
            "settings.getIoStatus",
            "settings.getOverclockMode",
        ] {
            assert_eq!(
                MethodClass::classify(method),
                MethodClass::Safe,
                "expected {method} to be Safe"
            );
        }
    }

    #[test]
    fn test_classify_mutating_methods() {
        for method in &[
            "download.start",
            "download.pause",
            "download.resume",
            "download.cancel",
            "download.remove",
            "download.purge",
            "settings.save",
            "bt.setSpeedLimit",
            "bt.previewTorrent",
            "bt.updateFiles",
            "cdn.test",
            "cdn.apply",
            "cdn.clear",
            "cdn.cancel",
            "settings.toggleGameMode",
            "settings.toggleOverclockMode",
            "download.openInExplorer",
            "settings.fetchTrackerList",
        ] {
            assert_eq!(
                MethodClass::classify(method),
                MethodClass::Mutating,
                "expected {method} to be Mutating"
            );
        }
    }

    #[test]
    fn test_unknown_method_is_mutating() {
        assert_eq!(
            MethodClass::classify("some.random.method"),
            MethodClass::Mutating
        );
    }

    #[test]
    fn test_ws_rate_limiter_allows_safe() {
        let lim = WsRateLimiter::new();
        lim.register("conn1");
        assert!(lim.check("conn1", MethodClass::Safe).is_ok());
    }

    #[test]
    fn test_ws_rate_limiter_allows_mutating() {
        let lim = WsRateLimiter::new();
        lim.register("conn1");
        assert!(lim.check("conn1", MethodClass::Mutating).is_ok());
    }

    #[test]
    fn test_register_and_unregister_noop() {
        let lim = WsRateLimiter::new();
        lim.register("test");
        lim.unregister("test");
        // After unregister, the per-connection limiters are dropped,
        // so check should fail
        assert!(lim.check("test", MethodClass::Safe).is_err());
    }
}
