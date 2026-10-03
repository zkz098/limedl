use std::path::{Path, PathBuf};
use std::sync::Arc;
use parking_lot::RwLock;

use crate::error::Result;
use crate::settings::{load_settings, normalize_settings, persist_settings};
use crate::types::AppSettings;

/// Central service for loading, updating, normalizing, and persisting application settings.
/// Serves as the single source of truth for configuration across all backends and interfaces.
#[derive(Clone)]
pub struct SettingsService {
    settings_path: PathBuf,
    settings: Arc<RwLock<AppSettings>>,
    /// Serializes read-modify-write updates. Without it two concurrent saves can
    /// each clone the same base and the later write silently drops the other's
    /// change.
    update_lock: Arc<tokio::sync::Mutex<()>>,
}

impl SettingsService {
    pub fn new(settings_path: PathBuf) -> Result<Self> {
        let initial = load_settings(&settings_path)?;
        Ok(Self {
            settings_path,
            settings: Arc::new(RwLock::new(initial)),
            update_lock: Arc::new(tokio::sync::Mutex::new(())),
        })
    }

    /// Read the current settings asynchronously.
    pub async fn get(&self) -> AppSettings {
        self.settings.read().clone()
    }

    /// Read the current settings in a blocking context.
    pub fn get_blocking(&self) -> AppSettings {
        self.settings.read().clone()
    }

    /// Normalize, persist, and update in-memory settings.
    pub async fn update(&self, new_settings: &AppSettings) -> Result<AppSettings> {
        let _guard = self.update_lock.lock().await;
        self.persist(new_settings.clone()).await
    }

    /// Transactional update: `mutate` sees the *current persisted* settings and
    /// edits them in place. Serialized against every other `update`/`update_with`,
    /// so two concurrent saves can no longer each start from the same stale base.
    pub async fn update_with<F>(&self, mutate: F) -> Result<AppSettings>
    where
        F: FnOnce(&mut AppSettings) -> Result<()>,
    {
        let _guard = self.update_lock.lock().await;
        let mut candidate = self.settings.read().clone();
        mutate(&mut candidate)?;
        self.persist(candidate).await
    }

    async fn persist(&self, new_settings: AppSettings) -> Result<AppSettings> {
        let normalized = normalize_settings(new_settings)?;
        persist_settings(&self.settings_path, &normalized).await?;
        let mut w = self.settings.write();
        *w = normalized.clone();
        Ok(normalized)
    }

    /// Reset settings to defaults and persist.
    pub async fn factory_reset(&self) -> Result<AppSettings> {
        let defaults = AppSettings::default();
        self.update(&defaults).await
    }

    /// Returns the default download directory if non-empty.
    pub async fn default_download_dir(&self) -> Option<String> {
        let dir = self.settings.read().download.default_download_dir.clone();
        if dir.is_empty() {
            None
        } else {
            Some(dir)
        }
    }

    /// Returns the settings file path.
    pub fn settings_path(&self) -> &Path {
        &self.settings_path
    }

    /// Get shared Arc reference to the inner RwLock<AppSettings>.
    pub fn inner(&self) -> Arc<RwLock<AppSettings>> {
        self.settings.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::DownloadError;

    fn service(dir: &Path) -> SettingsService {
        SettingsService::new(dir.join("settings.json")).expect("settings service")
    }

    #[tokio::test]
    async fn update_with_sees_the_latest_base_and_survives_concurrency() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let service = Arc::new(service(tmp.path()));
        let base = service.get_blocking().download.default_max_retries;

        // Without the update lock these ten read-modify-write cycles would each
        // start from the same base and the final count would be < 10.
        let mut handles = Vec::new();
        for _ in 0..10 {
            let service = service.clone();
            handles.push(tokio::spawn(async move {
                service
                    .update_with(|settings| {
                        settings.download.default_max_retries += 1;
                        Ok(())
                    })
                    .await
            }));
        }
        for handle in handles {
            handle.await.expect("task did not panic").expect("update");
        }

        assert_eq!(service.get_blocking().download.default_max_retries, base + 10);
    }

    #[tokio::test]
    async fn update_with_leaves_settings_untouched_on_error() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let service = service(tmp.path());

        let result = service
            .update_with(|settings| {
                settings.download.default_max_retries = 7;
                Err(DownloadError::InvalidRequest("rejected".into()))
            })
            .await;

        assert!(result.is_err(), "the closure error must propagate");
        assert_eq!(
            service.get_blocking().download.default_max_retries,
            AppSettings::default().download.default_max_retries,
            "a rejected transaction must not persist its partial mutation"
        );
    }
}
