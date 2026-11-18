//! `redb` repository for HTTP/3 `Alt-Svc` cache entries (`alt_svc.redb`).

use anyhow::{Context, Result};
use redb::{ReadableTable, ReadableTableMetadata};

use super::AltSvcRecord;
use crate::database::Database;
use crate::database::schema::ALT_SVC_CACHE;

impl Database {
    /// Load all cached Alt-Svc records from `alt_svc.redb`.
    pub fn load_all_alt_svc(&self) -> Result<Vec<AltSvcRecord>> {
        let tx = self.begin_alt_svc_read()?;
        let table = tx.open_table(ALT_SVC_CACHE)?;
        let mut results = Vec::with_capacity(table.len()? as usize);

        for entry in table.iter()? {
            let (k, v) = entry?;
            match serde_json::from_slice::<AltSvcRecord>(v.value()) {
                Ok(record) => results.push(record),
                Err(err) => {
                    let (host, port, proto) = k.value();
                    tracing::warn!(
                        "skipping unreadable alt_svc_cache row ({host}:{port}, {proto}): {err}"
                    );
                }
            }
        }
        Ok(results)
    }

    /// Insert or update an Alt-Svc record.
    pub fn upsert_alt_svc(&self, record: &AltSvcRecord) -> Result<()> {
        let bytes = serde_json::to_vec(record).context("failed to serialize alt_svc record")?;
        let tx = self.begin_alt_svc_write()?;
        {
            let mut table = tx.open_table(ALT_SVC_CACHE)?;
            let key = (
                record.origin_host.as_str(),
                record.origin_port,
                record.protocol.as_str(),
            );
            table
                .insert(key, bytes.as_slice())
                .context("failed to upsert alt_svc_cache")?;
        }
        tx.commit()
            .context("failed to commit alt_svc_cache upsert")?;
        Ok(())
    }

    /// Clear all Alt-Svc entries for a given origin authority.
    pub fn clear_alt_svc_for_origin(&self, origin_host: &str, origin_port: u16) -> Result<()> {
        let tx = self.begin_alt_svc_write()?;
        {
            let mut table = tx.open_table(ALT_SVC_CACHE)?;
            let keys: Vec<(String, u16, String)> = table
                .iter()?
                .filter_map(|res| {
                    let (k, _) = res.ok()?;
                    let (host, port, proto) = k.value();
                    if host == origin_host && port == origin_port {
                        Some((host.to_owned(), port, proto.to_owned()))
                    } else {
                        None
                    }
                })
                .collect();

            for (host, port, proto) in keys {
                let _ = table.remove((host.as_str(), port, proto.as_str()))?;
            }
        }
        tx.commit()
            .context("failed to clear alt_svc_cache for origin")?;
        Ok(())
    }

    /// Delete all expired Alt-Svc entries.
    pub fn clean_expired_alt_svc(&self, now_ms: u64) -> Result<usize> {
        let tx = self.begin_alt_svc_write()?;
        let mut deleted = 0usize;
        {
            let mut table = tx.open_table(ALT_SVC_CACHE)?;
            let expired_keys: Vec<(String, u16, String)> = table
                .iter()?
                .filter_map(|res| {
                    let (k, v) = res.ok()?;
                    let record: AltSvcRecord = serde_json::from_slice(v.value()).ok()?;
                    if record.expires_at_ms <= now_ms {
                        let (host, port, proto) = k.value();
                        Some((host.to_owned(), port, proto.to_owned()))
                    } else {
                        None
                    }
                })
                .collect();

            for (host, port, proto) in expired_keys {
                if table
                    .remove((host.as_str(), port, proto.as_str()))?
                    .is_some()
                {
                    deleted += 1;
                }
            }
        }
        tx.commit()
            .context("failed to clean expired alt_svc_cache")?;
        Ok(deleted)
    }
}
