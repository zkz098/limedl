use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use redb::{
    DatabaseError, ReadTransaction, ReadableDatabase, ReadableTable, StorageError, WriteTransaction,
};

use super::schema::{
    ACTIVE_DOWNLOADS, ALT_SVC_CACHE, BT_TASKS, CHUNK_PROGRESS, META, SCHEMA_VERSION,
    TERMINAL_DOWNLOADS,
};
use super::writer::WriteActorHandle;

/// Persistent store backed by `redb`.
///
/// Physically separates:
/// - `core_db` (`downloads.redb`): active/terminal task manifests and compact chunk progress,
///   driven by a single-writer persistence actor thread.
/// - `bt_db` (`bt_tasks.redb`): ephemeral BT task summary cache (reset on corruption).
/// - `alt_svc_db` (`alt_svc.redb`): ephemeral HTTP/3 Alt-Svc cache (reset on corruption).
pub struct Database {
    core_db: Arc<redb::Database>,
    writer: WriteActorHandle,
    bt_db: Arc<redb::Database>,
    alt_svc_db: Arc<redb::Database>,
    quarantined_path: Option<PathBuf>,
}

impl Database {
    /// Open (or create) the primary `redb` store at `path` along with its
    /// isolated ephemeral cache databases.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("failed to create database directory {}", parent.display())
            })?;
        }

        let (core_db, quarantined_path) = open_or_quarantine_core(path)?;
        init_core_tables(&core_db)?;

        let (bt_path, alt_svc_path) = cache_paths_for(path);
        let bt_db = open_or_reset_cache(&bt_path)?;
        init_bt_tables(&bt_db)?;

        let alt_svc_db = open_or_reset_cache(&alt_svc_path)?;
        init_alt_svc_tables(&alt_svc_db)?;

        let core_db = Arc::new(core_db);
        let writer = WriteActorHandle::spawn(Arc::clone(&core_db))?;

        Ok(Self {
            core_db,
            writer,
            bt_db: Arc::new(bt_db),
            alt_svc_db: Arc::new(alt_svc_db),
            quarantined_path,
        })
    }

    /// Create an in-memory `redb` database for testing.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn open_in_memory() -> Result<Self> {
        let core_db = redb::Database::builder()
            .create_with_backend(redb::backends::InMemoryBackend::new())
            .context("failed to create in-memory core database")?;
        init_core_tables(&core_db)?;

        let bt_db = redb::Database::builder()
            .create_with_backend(redb::backends::InMemoryBackend::new())
            .context("failed to create in-memory bt cache database")?;
        init_bt_tables(&bt_db)?;

        let alt_svc_db = redb::Database::builder()
            .create_with_backend(redb::backends::InMemoryBackend::new())
            .context("failed to create in-memory alt-svc cache database")?;
        init_alt_svc_tables(&alt_svc_db)?;

        let core_db = Arc::new(core_db);
        let writer = WriteActorHandle::spawn(Arc::clone(&core_db))?;

        Ok(Self {
            core_db,
            writer,
            bt_db: Arc::new(bt_db),
            alt_svc_db: Arc::new(alt_svc_db),
            quarantined_path: None,
        })
    }

    /// If a corrupt core database file was moved aside when opening this
    /// instance, returns the path where the corrupt copy was preserved.
    pub fn quarantined_path(&self) -> Option<&Path> {
        self.quarantined_path.as_deref()
    }

    pub(crate) fn writer(&self) -> &WriteActorHandle {
        &self.writer
    }

    pub(crate) fn begin_read(&self) -> Result<ReadTransaction> {
        self.core_db
            .begin_read()
            .context("failed to begin read transaction")
    }

    pub(crate) fn begin_bt_read(&self) -> Result<ReadTransaction> {
        self.bt_db
            .begin_read()
            .context("failed to begin bt cache read transaction")
    }

    pub(crate) fn begin_bt_write(&self) -> Result<WriteTransaction> {
        self.bt_db
            .begin_write()
            .context("failed to begin bt cache write transaction")
    }

    pub(crate) fn begin_alt_svc_read(&self) -> Result<ReadTransaction> {
        self.alt_svc_db
            .begin_read()
            .context("failed to begin alt-svc cache read transaction")
    }

    pub(crate) fn begin_alt_svc_write(&self) -> Result<WriteTransaction> {
        self.alt_svc_db
            .begin_write()
            .context("failed to begin alt-svc cache write transaction")
    }

    /// Execute a blocking database operation on Tokio's blocking thread pool.
    pub async fn run_blocking<F, T>(self: &Arc<Self>, f: F) -> Result<T>
    where
        F: FnOnce(&Database) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let db = Arc::clone(self);
        tokio::task::spawn_blocking(move || f(&db))
            .await
            .context("database blocking task panicked")?
    }

    /// Flush all queued progress updates and commit a durable barrier on clean
    /// shutdown so any prior `Durability::None` 300 ms progress writes are
    /// persisted to disk before exit.
    pub fn shutdown(&self) -> Result<()> {
        self.writer.shutdown()
    }

    /// Async wrapper for [`Self::shutdown`] that runs on Tokio's blocking pool.
    pub async fn shutdown_async(self: &Arc<Self>) -> Result<()> {
        self.run_blocking(Database::shutdown).await
    }
}

fn init_core_tables(db: &redb::Database) -> Result<()> {
    let tx = db
        .begin_write()
        .context("failed to begin core table init")?;
    {
        let mut meta = tx.open_table(META)?;
        if let Some(existing) = meta.get("schema_version")? {
            let version = existing.value();
            if version != SCHEMA_VERSION {
                bail!(
                    "unsupported downloads.redb schema version {version} (expected {SCHEMA_VERSION})"
                );
            }
        } else {
            meta.insert("schema_version", SCHEMA_VERSION)?;
        }
        let _ = tx.open_table(ACTIVE_DOWNLOADS)?;
        let _ = tx.open_table(TERMINAL_DOWNLOADS)?;
        let _ = tx.open_table(CHUNK_PROGRESS)?;
    }
    tx.commit().context("failed to commit core table init")?;
    Ok(())
}

fn init_bt_tables(db: &redb::Database) -> Result<()> {
    let tx = db.begin_write().context("failed to begin bt table init")?;
    {
        let _ = tx.open_table(BT_TASKS)?;
    }
    tx.commit().context("failed to commit bt table init")?;
    Ok(())
}

fn init_alt_svc_tables(db: &redb::Database) -> Result<()> {
    let tx = db
        .begin_write()
        .context("failed to begin alt-svc table init")?;
    {
        let _ = tx.open_table(ALT_SVC_CACHE)?;
    }
    tx.commit().context("failed to commit alt-svc table init")?;
    Ok(())
}

fn cache_paths_for(core_path: &Path) -> (PathBuf, PathBuf) {
    if core_path.file_name().and_then(|n| n.to_str()) == Some("downloads.redb")
        && let Some(parent) = core_path.parent()
    {
        return (parent.join("bt_tasks.redb"), parent.join("alt_svc.redb"));
    }
    (
        with_suffix(core_path, ".bt.redb"),
        with_suffix(core_path, ".alt_svc.redb"),
    )
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

fn quarantine_path(path: &Path) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or(0);
    with_suffix(path, &format!(".corrupt-{stamp}"))
}

fn is_lock_contention(error: &DatabaseError) -> bool {
    matches!(error, DatabaseError::DatabaseAlreadyOpen)
        || error.to_string().to_ascii_lowercase().contains("locked")
}

fn is_corruption_error(error: &DatabaseError) -> bool {
    match error {
        DatabaseError::RepairAborted | DatabaseError::UpgradeRequired(_) => true,
        DatabaseError::Storage(StorageError::Corrupted(_)) => true,
        DatabaseError::Storage(StorageError::Io(io_err)) => matches!(
            io_err.kind(),
            std::io::ErrorKind::UnexpectedEof | std::io::ErrorKind::InvalidData
        ),
        _ => false,
    }
}

fn open_redb_with_lock_retry(path: &Path) -> std::result::Result<redb::Database, DatabaseError> {
    const MAX_ATTEMPTS: usize = 20;
    const RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(10);

    for attempt in 0..MAX_ATTEMPTS {
        match redb::Database::open(path) {
            Ok(db) => return Ok(db),
            Err(err) if is_lock_contention(&err) && attempt + 1 < MAX_ATTEMPTS => {
                std::thread::sleep(RETRY_DELAY);
            }
            Err(err) => return Err(err),
        }
    }
    unreachable!()
}

fn open_or_quarantine_core(path: &Path) -> Result<(redb::Database, Option<PathBuf>)> {
    let exists_non_empty = std::fs::metadata(path)
        .map(|m| m.len() > 0)
        .unwrap_or(false);
    if !exists_non_empty {
        let db = redb::Database::create(path)
            .with_context(|| format!("failed to create database at {}", path.display()))?;
        return Ok((db, None));
    }

    match open_redb_with_lock_retry(path) {
        Ok(db) => Ok((db, None)),
        Err(err) if is_lock_contention(&err) => {
            Err(err).with_context(|| format!("failed to open database at {}", path.display()))
        }
        Err(err) if is_corruption_error(&err) => {
            tracing::error!(
                "database at {} cannot be opened ({err}); moving it aside",
                path.display()
            );
            let destination = quarantine_path(path);
            std::fs::rename(path, &destination).with_context(|| {
                format!(
                    "failed to move the corrupt database {} to {}",
                    path.display(),
                    destination.display()
                )
            })?;
            tracing::error!(
                "the corrupt database was preserved at {} — download task history was reset, \
                 files on disk are untouched",
                destination.display()
            );
            let db = redb::Database::create(path)
                .with_context(|| format!("failed to recreate database at {}", path.display()))?;
            Ok((db, Some(destination)))
        }
        Err(err) => {
            // Other unreadable/invalid file format errors also warrant quarantine.
            tracing::error!(
                "database at {} failed to open ({err}); moving it aside",
                path.display()
            );
            let destination = quarantine_path(path);
            std::fs::rename(path, &destination).with_context(|| {
                format!(
                    "failed to move the corrupt database {} to {}",
                    path.display(),
                    destination.display()
                )
            })?;
            let db = redb::Database::create(path)
                .with_context(|| format!("failed to recreate database at {}", path.display()))?;
            Ok((db, Some(destination)))
        }
    }
}

fn open_or_reset_cache(path: &Path) -> Result<redb::Database> {
    let exists_non_empty = std::fs::metadata(path)
        .map(|m| m.len() > 0)
        .unwrap_or(false);
    if !exists_non_empty {
        return redb::Database::create(path)
            .with_context(|| format!("failed to create cache database at {}", path.display()));
    }

    match open_redb_with_lock_retry(path) {
        Ok(db) => Ok(db),
        Err(err) if is_lock_contention(&err) => {
            Err(err).with_context(|| format!("failed to open cache database at {}", path.display()))
        }
        Err(err) => {
            tracing::warn!(
                "ephemeral cache database at {} is unreadable ({err}); resetting it",
                path.display()
            );
            let _ = std::fs::remove_file(path);
            redb::Database::create(path)
                .with_context(|| format!("failed to recreate cache database at {}", path.display()))
        }
    }
}
