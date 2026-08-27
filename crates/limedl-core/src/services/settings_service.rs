use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::Result;
use crate::settings::{load_settings, normalize_settings, persist_settings};
use crate::types::AppSettings;

/// Central service for loading, updating, normalizing, and persisting application settings.
/// Serves as the single source of truth for configuration across all backends and interfaces.
#[derive(Clone)]
pub struct SettingsService {
    settings_path: PathBuf,
    settings: Arc<RwLock<AppSettings>>,
}

impl SettingsService {
    pub fn new(settings_path: PathBuf) -> Result<Self> {
        let initial = load_settings(&settings_path)?;
        Ok(Self {
            settings_path,
            settings: Arc::new(RwLock::new(initial)),
        })
    }

    /// Read the current settings asynchronously.
    pub async fn get(&self) -> AppSettings {
        self.settings.read().await.clone()
    }

    /// Read the current settings in a blocking context.
    pub fn get_blocking(&self) -> AppSettings {
        tokio::task::block_in_place(|| self.settings.blocking_read().clone())
    }

    /// Normalize, persist, and update in-memory settings.
    pub async fn update(&self, new_settings: &AppSettings) -> Result<AppSettings> {
        let normalized = normalize_settings(new_settings.clone())?;
        persist_settings(&self.settings_path, &normalized).await?;
        let mut w = self.settings.write().await;
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
        let dir = self.settings.read().await.download.default_download_dir.clone();
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
