//! SQLite repository for HTTP/3 `Alt-Svc` cache entries.

use anyhow::{Context, Result};
use rusqlite::params;

use super::connection::Database;
use crate::http::alt_svc::AltSvcRecord;

impl Database {
    /// Load all cached Alt-Svc records from SQLite.
    pub fn load_all_alt_svc(&self) -> Result<Vec<AltSvcRecord>> {
        let conn = self.lock_read();
        let mut stmt = conn
            .prepare(
                "SELECT origin_host, origin_port, protocol, alt_host, alt_port,
                        expires_at_ms, persist, fail_count, broken_until_ms
                 FROM alt_svc_cache",
            )
            .context("failed to prepare select alt_svc_cache query")?;

        let rows = stmt
            .query_map([], |row| {
                let persist_int: i64 = row.get(6)?;
                Ok(AltSvcRecord {
                    origin_host: row.get(0)?,
                    origin_port: row.get(1)?,
                    protocol: row.get(2)?,
                    alt_host: row.get(3)?,
                    alt_port: row.get(4)?,
                    expires_at_ms: row.get::<_, i64>(5)? as u64,
                    persist: persist_int != 0,
                    fail_count: row.get::<_, i64>(7)? as u32,
                    broken_until_ms: row.get::<_, i64>(8)? as u64,
                })
            })
            .context("failed to query alt_svc_cache rows")?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.context("failed to read alt_svc_cache row")?);
        }
        Ok(results)
    }

    /// Insert or update an Alt-Svc record.
    pub fn upsert_alt_svc(&self, record: &AltSvcRecord) -> Result<()> {
        let conn = self.lock_write();
        let persist_int = if record.persist { 1i64 } else { 0i64 };

        conn.execute(
            "INSERT INTO alt_svc_cache (
                origin_host, origin_port, protocol, alt_host, alt_port,
                expires_at_ms, persist, fail_count, broken_until_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(origin_host, origin_port, protocol) DO UPDATE SET
                alt_host = excluded.alt_host,
                alt_port = excluded.alt_port,
                expires_at_ms = excluded.expires_at_ms,
                persist = excluded.persist,
                fail_count = excluded.fail_count,
                broken_until_ms = excluded.broken_until_ms",
            params![
                record.origin_host,
                record.origin_port,
                record.protocol,
                record.alt_host,
                record.alt_port,
                record.expires_at_ms as i64,
                persist_int,
                record.fail_count as i64,
                record.broken_until_ms as i64,
            ],
        )
        .context("failed to upsert alt_svc_cache")?;

        Ok(())
    }

    /// Clear all Alt-Svc entries for a given origin authority.
    pub fn clear_alt_svc_for_origin(&self, origin_host: &str, origin_port: u16) -> Result<()> {
        let conn = self.lock_write();
        conn.execute(
            "DELETE FROM alt_svc_cache WHERE origin_host = ?1 AND origin_port = ?2",
            params![origin_host, origin_port],
        )
        .context("failed to clear alt_svc_cache for origin")?;
        Ok(())
    }

    /// Delete all expired Alt-Svc entries from SQLite.
    pub fn clean_expired_alt_svc(&self, now_ms: u64) -> Result<usize> {
        let conn = self.lock_write();
        let deleted = conn
            .execute(
                "DELETE FROM alt_svc_cache WHERE expires_at_ms <= ?1",
                params![now_ms as i64],
            )
            .context("failed to clean expired alt_svc_cache")?;
        Ok(deleted)
    }
}
