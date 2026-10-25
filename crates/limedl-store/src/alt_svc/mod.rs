//! HTTP/3 (QUIC) `Alt-Svc` (RFC 7838 / RFC 9114) cross-task persistent cache.
//!
//! Provides parsing for `Alt-Svc` response headers, an in-memory L1 cache with
//! broken-service circuit breaking and exponential backoff, and backing SQLite
//! persistence for cross-task, cross-session discovery.
//!
//! This is a top-level module rather than a child of `http` because it owns its
//! own SQLite repository ([`repo`]): inside `http` the HTTP utility layer both
//! depended on the database layer and was depended on *by* it (through
//! `AltSvcRecord`), the `database <-> http` cycle the module-boundary guard
//! rejects.

use std::sync::Arc;

use dashmap::DashMap;
use tracing::{debug, info, warn};

use crate::database::Database;
use crate::error::Result;

/// SQLite repository: `impl Database` for the `alt_svc_cache` table.
pub mod repo;

#[cfg(test)]
mod repo_tests;

/// A persisted or cached alternative service record.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AltSvcRecord {
    pub origin_host: String,
    pub origin_port: u16,
    pub protocol: String,
    pub alt_host: String,
    pub alt_port: u16,
    pub expires_at_ms: u64,
    pub persist: bool,
    pub fail_count: u32,
    pub broken_until_ms: u64,
}

impl AltSvcRecord {
    /// Returns true if this record is within its validity period and not currently
    /// in a broken-circuit backoff penalty window.
    pub(crate) fn is_valid(&self, now_ms: u64) -> bool {
        now_ms < self.expires_at_ms && now_ms >= self.broken_until_ms
    }

    /// Mark the alternative service as broken with exponential backoff penalty.
    pub fn mark_broken(&mut self, now_ms: u64) {
        self.fail_count = self.fail_count.saturating_add(1);
        let penalty_secs: u64 = match self.fail_count {
            1 => 5 * 60,       // 5 minutes
            2 => 10 * 60,      // 10 minutes
            3 => 30 * 60,      // 30 minutes
            _ => 24 * 60 * 60, // 24 hours
        };
        self.broken_until_ms = now_ms.saturating_add(penalty_secs.saturating_mul(1000));
    }

    /// Reset failure tracking when an alternative connection succeeds.
    pub fn mark_success(&mut self) {
        self.fail_count = 0;
        self.broken_until_ms = 0;
    }
}

/// Action parsed from an `Alt-Svc` header.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AltSvcAction {
    /// Clear all alternative services for the origin (`Alt-Svc: clear`).
    Clear,
    /// One or more advertised alternative services.
    Entries(Vec<ParsedAltSvcEntry>),
}

/// An entry parsed from an `Alt-Svc` header.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ParsedAltSvcEntry {
    pub protocol: String,
    pub host: Option<String>,
    pub port: u16,
    pub max_age_secs: u64,
    pub persist: bool,
}

/// Helper to extract origin host and port from an HTTPS URL.
pub(crate) fn extract_https_origin(url: &str) -> Option<(String, u16)> {
    let parsed = url::Url::parse(url).ok()?;
    if parsed.scheme() != "https" {
        return None;
    }
    let host = parsed.host_str()?.to_string();
    let port = parsed.port_or_known_default().unwrap_or(443);
    Some((host, port))
}

/// Parse an RFC 7838 `Alt-Svc` header value.
///
/// Example inputs:
/// - `clear`
/// - `h3=":443"; ma=2592000; persist=1`
/// - `h3="alt.example.com:443"; ma=86400, h3-29=":443"; ma=86400`
pub(crate) fn parse_alt_svc_header(header: &str) -> AltSvcAction {
    let trimmed = header.trim();
    if trimmed.eq_ignore_ascii_case("clear") {
        return AltSvcAction::Clear;
    }

    let mut entries = Vec::new();

    // Split multiple comma-separated alternative services
    for item in split_alt_svc_entries(trimmed) {
        if let Some(entry) = parse_single_entry(item) {
            entries.push(entry);
        }
    }

    AltSvcAction::Entries(entries)
}

/// Split comma-separated entries, ignoring commas within quoted strings.
fn split_alt_svc_entries(input: &str) -> Vec<&str> {
    let mut entries = Vec::new();
    let mut in_quotes = false;
    let mut start = 0;

    for (idx, ch) in input.char_indices() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                let piece = input[start..idx].trim();
                if !piece.is_empty() {
                    entries.push(piece);
                }
                start = idx + 1;
            }
            _ => {}
        }
    }

    let tail = input[start..].trim();
    if !tail.is_empty() {
        entries.push(tail);
    }

    entries
}

/// Parse a single `proto="alt-authority"[; param=val]*` item.
fn parse_single_entry(item: &str) -> Option<ParsedAltSvcEntry> {
    let mut parts = item.split(';');
    let proto_and_auth = parts.next()?.trim();

    let mut proto_parts = proto_and_auth.splitn(2, '=');
    let protocol = proto_parts.next()?.trim().to_lowercase();
    let authority = proto_parts.next()?.trim();

    // Strip quotes around authority if present
    let authority =
        if authority.starts_with('"') && authority.ends_with('"') && authority.len() >= 2 {
            &authority[1..authority.len() - 1]
        } else {
            authority
        };

    // Parse authority into host and port
    let (host, port) = if let Some(stripped) = authority.strip_prefix(':') {
        let port: u16 = stripped.parse().ok()?;
        (None, port)
    } else if let Some((h, p)) = authority.rsplit_once(':') {
        let port: u16 = p.parse().ok()?;
        let host = if h.is_empty() {
            None
        } else {
            Some(h.to_string())
        };
        (host, port)
    } else {
        return None;
    };

    let mut max_age_secs = 86400; // Default: 24 hours
    let mut persist = false;

    for param in parts {
        let param = param.trim();
        if let Some((k, v)) = param.split_once('=') {
            let k = k.trim().to_lowercase();
            let v = v.trim().trim_matches('"');
            if k == "ma" {
                if let Ok(secs) = v.parse::<u64>() {
                    max_age_secs = secs;
                }
            } else if k == "persist" && v == "1" {
                persist = true;
            }
        }
    }

    Some(ParsedAltSvcEntry {
        protocol,
        host,
        port,
        max_age_secs,
        persist,
    })
}

/// Manages L1 in-memory Alt-Svc cache and L2 SQLite persistence.
pub struct AltSvcManager {
    cache: DashMap<(String, u16), Vec<AltSvcRecord>>,
    db: Arc<Database>,
}

impl AltSvcManager {
    /// Create a new AltSvcManager backed by the database.
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            cache: DashMap::new(),
            db,
        }
    }

    /// Preload all unexpired records from SQLite into the in-memory cache.
    pub fn preload_from_db(&self) -> Result<()> {
        let records = self.db.load_all_alt_svc()?;
        let now_ms = crate::now_ms();
        for record in records {
            if record.expires_at_ms > now_ms {
                let key = (record.origin_host.clone(), record.origin_port);
                self.cache.entry(key).or_default().push(record);
            }
        }
        Ok(())
    }

    /// Look up a valid (unexpired and not broken) HTTP/3 alternative service.
    ///
    /// Prefers standard `h3`, falling back to draft protocols like `h3-29`.
    pub(crate) fn get_valid_alt_svc(
        &self,
        origin_host: &str,
        origin_port: u16,
        now_ms: u64,
    ) -> Option<AltSvcRecord> {
        let key = (origin_host.to_string(), origin_port);
        let entry = self.cache.get(&key)?;

        // Find standard "h3" first
        if let Some(rec) = entry
            .iter()
            .find(|r| r.protocol == "h3" && r.is_valid(now_ms))
        {
            return Some(rec.clone());
        }

        // Fall back to draft H3 (e.g. "h3-29")
        entry
            .iter()
            .find(|r| r.protocol.starts_with("h3") && r.is_valid(now_ms))
            .cloned()
    }

    /// Check whether HTTP/3 should be attempted for the given URL, respecting
    /// settings, proxy configuration, and current circuit breaker status.
    pub fn should_attempt_http3(
        &self,
        url: &str,
        proxy_mode: crate::types::ProxyMode,
        http3_enabled: bool,
        now_ms: u64,
    ) -> Option<(String, u16)> {
        if !http3_enabled || proxy_mode != crate::types::ProxyMode::Disabled {
            return None;
        }
        let (host, port) = extract_https_origin(url)?;
        if self.get_valid_alt_svc(&host, port, now_ms).is_some() {
            Some((host, port))
        } else {
            None
        }
    }

    /// Record Alt-Svc header directly from response headers and URL if valid.
    pub fn record_from_response(&self, url: &str, headers: &http::header::HeaderMap, now_ms: u64) {
        if let Some(alt_val) = headers.get(http::header::ALT_SVC)
            && let Ok(alt_str) = alt_val.to_str()
            && let Some((host, port)) = extract_https_origin(url)
        {
            self.record_alt_svc_header(&host, port, alt_str, now_ms);
        }
    }

    /// Ingest an `Alt-Svc` response header for an origin authority.
    pub(crate) fn record_alt_svc_header(
        &self,
        origin_host: &str,
        origin_port: u16,
        header_value: &str,
        now_ms: u64,
    ) {
        let key = (origin_host.to_string(), origin_port);
        match parse_alt_svc_header(header_value) {
            AltSvcAction::Clear => self.clear_origin(&key, origin_host, origin_port),
            AltSvcAction::Entries(entries) => {
                let records = self.collect_records(&key, origin_host, origin_port, entries, now_ms);
                if !records.is_empty() {
                    self.store_records(key, records);
                }
            }
        }
    }

    /// Drop every Alt-Svc entry for one origin, in memory and in SQLite.
    fn clear_origin(&self, key: &(String, u16), origin_host: &str, origin_port: u16) {
        debug!("Clearing Alt-Svc entries for {origin_host}:{origin_port}");
        self.cache.remove(key);
        let db = self.db.clone();
        let host = origin_host.to_string();
        tokio::spawn(async move {
            if let Err(e) = db.clear_alt_svc_for_origin(&host, origin_port) {
                warn!("Failed to clear Alt-Svc in db for {host}:{origin_port}: {e}");
            }
        });
    }

    /// Build the records worth caching from a parsed header. HTTP/3-only, and
    /// each record carries over the failure/broken state of the entry we already
    /// track for the same protocol and port.
    fn collect_records(
        &self,
        key: &(String, u16),
        origin_host: &str,
        origin_port: u16,
        entries: Vec<ParsedAltSvcEntry>,
        now_ms: u64,
    ) -> Vec<AltSvcRecord> {
        let mut records = Vec::new();
        for entry in entries {
            if !entry.protocol.starts_with("h3") {
                continue;
            }
            let (fail_count, broken_until_ms) =
                self.existing_failure_state(key, &entry.protocol, entry.port);
            records.push(AltSvcRecord {
                origin_host: origin_host.to_string(),
                origin_port,
                protocol: entry.protocol,
                alt_host: entry.host.unwrap_or_else(|| origin_host.to_string()),
                alt_port: entry.port,
                expires_at_ms: now_ms.saturating_add(entry.max_age_secs.saturating_mul(1000)),
                persist: entry.persist,
                fail_count,
                broken_until_ms,
            });
        }
        records
    }

    /// `fail_count`/`broken_until_ms` of an already-tracked entry for the same
    /// protocol and port, so a re-advertised service does not lose its backoff.
    fn existing_failure_state(
        &self,
        key: &(String, u16),
        protocol: &str,
        alt_port: u16,
    ) -> (u32, u64) {
        let Some(list) = self.cache.get(key) else {
            return (0, 0);
        };
        let Some(existing) = list
            .iter()
            .find(|r| r.protocol == protocol && r.alt_port == alt_port)
        else {
            return (0, 0);
        };
        (existing.fail_count, existing.broken_until_ms)
    }

    /// Merge `records` into the in-memory list (replacing same protocol+port)
    /// and persist them in the background.
    fn store_records(&self, key: (String, u16), records: Vec<AltSvcRecord>) {
        let mut list = self.cache.entry(key).or_default();
        for rec in &records {
            if let Some(pos) = list
                .iter()
                .position(|r| r.protocol == rec.protocol && r.alt_port == rec.alt_port)
            {
                list[pos] = rec.clone();
            } else {
                list.push(rec.clone());
            }
        }

        let db = self.db.clone();
        tokio::spawn(async move {
            for rec in records {
                if let Err(e) = db.upsert_alt_svc(&rec) {
                    warn!(
                        "Failed to persist Alt-Svc {}:{} -> {}:{}: {e}",
                        rec.origin_host, rec.origin_port, rec.alt_host, rec.alt_port
                    );
                }
            }
        });
    }

    /// Mark an alternative service as broken when an HTTP/3 connection/stream fails.
    pub fn mark_broken(&self, origin_host: &str, origin_port: u16, now_ms: u64) {
        let key = (origin_host.to_string(), origin_port);
        let mut to_persist = Vec::new();

        if let Some(mut entry) = self.cache.get_mut(&key) {
            for rec in entry.iter_mut() {
                if rec.protocol.starts_with("h3") {
                    rec.mark_broken(now_ms);
                    info!(
                        "Marked Alt-Svc {}:{} (h3) as broken until {} (fail count: {})",
                        origin_host, origin_port, rec.broken_until_ms, rec.fail_count
                    );
                    to_persist.push(rec.clone());
                }
            }
        }

        if !to_persist.is_empty() {
            let db = self.db.clone();
            tokio::spawn(async move {
                for rec in to_persist {
                    let _ = db.upsert_alt_svc(&rec);
                }
            });
        }
    }

    /// Reset failure tracking when an HTTP/3 connection succeeds.
    pub fn mark_success(&self, origin_host: &str, origin_port: u16) {
        let key = (origin_host.to_string(), origin_port);
        let mut to_persist = Vec::new();

        if let Some(mut entry) = self.cache.get_mut(&key) {
            for rec in entry.iter_mut() {
                if rec.protocol.starts_with("h3") && (rec.fail_count > 0 || rec.broken_until_ms > 0)
                {
                    rec.mark_success();
                    to_persist.push(rec.clone());
                }
            }
        }

        if !to_persist.is_empty() {
            let db = self.db.clone();
            tokio::spawn(async move {
                for rec in to_persist {
                    let _ = db.upsert_alt_svc(&rec);
                }
            });
        }
    }

    /// Remove expired records from in-memory cache and SQLite.
    pub fn clean_expired(&self, now_ms: u64) {
        self.cache.retain(|_, list| {
            list.retain(|rec| rec.expires_at_ms > now_ms);
            !list.is_empty()
        });

        let db = self.db.clone();
        tokio::spawn(async move {
            let _ = db.clean_expired_alt_svc(now_ms);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_alt_svc_clear() {
        assert_eq!(parse_alt_svc_header("clear"), AltSvcAction::Clear);
        assert_eq!(parse_alt_svc_header("CLEAR"), AltSvcAction::Clear);
        assert_eq!(parse_alt_svc_header("  clear  "), AltSvcAction::Clear);
    }

    #[test]
    fn parse_alt_svc_single_entry() {
        let action = parse_alt_svc_header(r#"h3=":443"; ma=86400; persist=1"#);
        match action {
            AltSvcAction::Entries(entries) => {
                assert_eq!(entries.len(), 1);
                assert_eq!(entries[0].protocol, "h3");
                assert_eq!(entries[0].host, None);
                assert_eq!(entries[0].port, 443);
                assert_eq!(entries[0].max_age_secs, 86400);
                assert!(entries[0].persist);
            }
            _ => panic!("expected AltSvcAction::Entries"),
        }
    }

    #[test]
    fn parse_alt_svc_multiple_entries() {
        let header = r#"h3=":443"; ma=2592000, h3-29="alt.example.com:8443"; ma=3600"#;
        let action = parse_alt_svc_header(header);
        match action {
            AltSvcAction::Entries(entries) => {
                assert_eq!(entries.len(), 2);
                assert_eq!(entries[0].protocol, "h3");
                assert_eq!(entries[0].host, None);
                assert_eq!(entries[0].port, 443);
                assert_eq!(entries[0].max_age_secs, 2592000);
                assert!(!entries[0].persist);

                assert_eq!(entries[1].protocol, "h3-29");
                assert_eq!(entries[1].host.as_deref(), Some("alt.example.com"));
                assert_eq!(entries[1].port, 8443);
                assert_eq!(entries[1].max_age_secs, 3600);
            }
            _ => panic!("expected AltSvcAction::Entries"),
        }
    }

    #[test]
    fn broken_backoff_progression() {
        let mut rec = AltSvcRecord {
            origin_host: "example.com".into(),
            origin_port: 443,
            protocol: "h3".into(),
            alt_host: "example.com".into(),
            alt_port: 443,
            expires_at_ms: 10_000_000,
            persist: false,
            fail_count: 0,
            broken_until_ms: 0,
        };

        let now = 1_000_000;
        assert!(rec.is_valid(now));

        // 1st failure: 5 min penalty
        rec.mark_broken(now);
        assert_eq!(rec.fail_count, 1);
        assert_eq!(rec.broken_until_ms, now + 300_000);
        assert!(!rec.is_valid(now));
        assert!(rec.is_valid(now + 300_001));

        // 2nd failure: 10 min penalty
        rec.mark_broken(now + 300_001);
        assert_eq!(rec.fail_count, 2);
        assert_eq!(rec.broken_until_ms, now + 300_001 + 600_000);

        // Success resets broken status
        rec.mark_success();
        assert_eq!(rec.fail_count, 0);
        assert_eq!(rec.broken_until_ms, 0);
        assert!(rec.is_valid(now));
    }

    #[tokio::test]
    async fn test_alt_svc_manager_flow() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let manager = AltSvcManager::new(db.clone());

        let url = "https://example.com/file.zip";
        let now = 1_000_000;

        // Initially no alt-svc
        assert!(
            manager
                .should_attempt_http3(url, crate::types::ProxyMode::Disabled, true, now)
                .is_none()
        );

        // Record header
        let mut headers = http::header::HeaderMap::new();
        headers.insert(
            http::header::ALT_SVC,
            http::header::HeaderValue::from_static(r#"h3=":443"; ma=3600; persist=1"#),
        );
        manager.record_from_response(url, &headers, now);

        // Now should attempt HTTP/3
        let target =
            manager.should_attempt_http3(url, crate::types::ProxyMode::Disabled, true, now);
        assert_eq!(target, Some(("example.com".into(), 443)));

        // If proxy is enabled, should bypass HTTP/3
        assert!(
            manager
                .should_attempt_http3(url, crate::types::ProxyMode::Manual, true, now)
                .is_none()
        );

        // If setting disabled, should bypass HTTP/3
        assert!(
            manager
                .should_attempt_http3(url, crate::types::ProxyMode::Disabled, false, now)
                .is_none()
        );

        // If HTTP URL (not HTTPS), should bypass HTTP/3
        assert!(
            manager
                .should_attempt_http3(
                    "http://example.com/file.zip",
                    crate::types::ProxyMode::Disabled,
                    true,
                    now
                )
                .is_none()
        );

        // Mark broken
        manager.mark_broken("example.com", 443, now);
        // Broken for 5 min (300_000 ms)
        assert!(
            manager
                .should_attempt_http3(url, crate::types::ProxyMode::Disabled, true, now + 1000)
                .is_none()
        );
        assert!(
            manager
                .should_attempt_http3(url, crate::types::ProxyMode::Disabled, true, now + 300_001)
                .is_some()
        );

        // Mark success
        manager.mark_success("example.com", 443);
        assert!(
            manager
                .should_attempt_http3(url, crate::types::ProxyMode::Disabled, true, now + 1000)
                .is_some()
        );

        // Expired after max_age
        assert!(
            manager
                .should_attempt_http3(
                    url,
                    crate::types::ProxyMode::Disabled,
                    true,
                    now + 3_600_001
                )
                .is_none()
        );
    }

    #[tokio::test]
    async fn test_alt_svc_cross_instance_persistence() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let _manager1 = AltSvcManager::new(db.clone());

        let rec = AltSvcRecord {
            origin_host: "test.org".into(),
            origin_port: 443,
            protocol: "h3".into(),
            alt_host: "test.org".into(),
            alt_port: 443,
            expires_at_ms: crate::now_ms() + 100_000,
            persist: true,
            fail_count: 0,
            broken_until_ms: 0,
        };
        db.upsert_alt_svc(&rec).unwrap();

        // Create manager2 sharing the same DB
        let manager2 = AltSvcManager::new(db.clone());
        assert!(
            manager2
                .should_attempt_http3(
                    "https://test.org/test",
                    crate::types::ProxyMode::Disabled,
                    true,
                    crate::now_ms()
                )
                .is_none()
        );

        // Preload from DB
        manager2.preload_from_db().unwrap();
        assert!(
            manager2
                .should_attempt_http3(
                    "https://test.org/test",
                    crate::types::ProxyMode::Disabled,
                    true,
                    crate::now_ms()
                )
                .is_some()
        );

        // Clean expired
        manager2.clean_expired(crate::now_ms() + 200_000);
        assert!(
            manager2
                .should_attempt_http3(
                    "https://test.org/test",
                    crate::types::ProxyMode::Disabled,
                    true,
                    crate::now_ms()
                )
                .is_none()
        );
    }
}
