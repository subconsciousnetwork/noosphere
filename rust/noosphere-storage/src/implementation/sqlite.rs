use crate::{
    config::{ConfigurableStorage, StorageConfig},
    storage::Storage,
    store::Store,
    SPHERE_DB_STORE_NAMES,
};
use anyhow::{anyhow, Error, Result};
use async_trait::async_trait;
use noosphere_common::ConditionalSend;
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::{rusqlite, SqliteConnectionManager};
use rusqlite::{params, OptionalExtension, Transaction};
use std::{
    fmt::Display,
    path::{Path, PathBuf},
    sync::Arc,
};

/// Within the directory provided to [SqliteStorage], the
/// name of the sqlite database file.
const SQLITE_DB_PATH: &str = "database.db";

/// Sqlite-backed [Storage] implementation.
#[derive(Clone, Debug)]
pub struct SqliteStorage {
    client: Arc<SqliteClient>,
}

impl SqliteStorage {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::with_config(path, StorageConfig::default())
    }

    pub fn with_config<P: AsRef<Path>>(path: P, storage_config: StorageConfig) -> Result<Self> {
        let client = Arc::new(SqliteClient::new(path.as_ref().to_owned(), storage_config)?);
        Ok(SqliteStorage { client })
    }
}

#[async_trait]
impl ConfigurableStorage for SqliteStorage {
    async fn open_with_config<P: AsRef<Path> + ConditionalSend>(
        path: P,
        storage_config: StorageConfig,
    ) -> Result<Self> {
        Self::with_config(path, storage_config)
    }
}

#[async_trait]
impl Storage for SqliteStorage {
    type BlockStore = SqliteStore;

    type KeyValueStore = SqliteStore;

    async fn get_block_store(&self, name: &str) -> Result<Self::BlockStore> {
        SqliteStore::new(self.client.clone(), name)
    }

    async fn get_key_value_store(&self, name: &str) -> Result<Self::KeyValueStore> {
        SqliteStore::new(self.client.clone(), name)
    }
}

#[derive(Clone)]
pub struct SqliteStore {
    client: Arc<SqliteClient>,
    queries: Arc<Queries>,
}

impl SqliteStore {
    fn new(client: Arc<SqliteClient>, table_name: &str) -> Result<Self> {
        let queries = Arc::new(Queries::new(table_name)?);
        client.transaction(|tx| {
            tx.prepare_cached(&Queries::generate_create_table_query(table_name))?
                .execute(())?;
            Ok(None)
        })?;
        Ok(SqliteStore { client, queries })
    }
}

#[async_trait]
impl Store for SqliteStore {
    async fn read(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        self.client.transaction(|tx| {
            let value = tx
                .prepare_cached(self.queries.read_query())?
                .query_row(params![key], |r| r.get(0))
                .optional()?;
            Ok(value)
        })
    }

    async fn write(&mut self, key: &[u8], bytes: &[u8]) -> Result<Option<Vec<u8>>> {
        self.client.transaction(|tx| {
            let previous = tx
                .prepare_cached(self.queries.read_query())?
                .query_row(params![key], |r| r.get(0))
                .optional()?;
            tx.prepare_cached(self.queries.write_query())?
                .execute(params![key, bytes])?;
            Ok(previous)
        })
    }

    async fn remove(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        self.client.transaction(|tx| {
            let previous = tx
                .prepare_cached(self.queries.read_query())?
                .query_row(params![key], |r| r.get(0))
                .optional()?;
            tx.prepare_cached(self.queries.delete_query())?
                .execute(params![key])?;
            Ok(previous)
        })
    }

    async fn flush(&self) -> Result<()> {
        Ok(())
    }
}

/// Manages a connection pool for a database.
#[derive(Debug)]
pub struct SqliteClient {
    pub(crate) path: PathBuf,
    connection_pool: Pool<SqliteConnectionManager>,
}

impl SqliteClient {
    /// Create a new [SqliteClient], opening a pool of connections
    /// for database at `path`.
    pub fn new(path: PathBuf, config: StorageConfig) -> Result<Self> {
        std::fs::create_dir_all(&path)?;
        let connection_pool = Pool::builder()
            .build(SqliteConnectionManager::file(path.join(SQLITE_DB_PATH)))
            .map_err(rusqlite_into_anyhow)?;
        let client = SqliteClient {
            path,
            connection_pool,
        };

        {
            let db = client.db()?;
            (match db
                .pragma_update_and_check(None, "journal_mode", "wal", |row| {
                    row.get::<usize, String>(0)
                })?
                .as_ref()
            {
                "wal" => Ok(()),
                _ => Err(anyhow!("Could not set journal_mode to WAL.")),
            })?;

            if let Some(memory_cache_limit) = config.memory_cache_limit {
                if memory_cache_limit >= 1024 {
                    // `cache_size` pragma takes either the number of pages to use in memory cache,
                    // or a negative value, indicating how many kibibytes should be used, deriving a page
                    // value from that limit. If we configure the page size after this, we should
                    // re-set the cache size.
                    // https://www.sqlite.org/pragma.html#pragma_cache_size
                    let kb = memory_cache_limit / 1024;
                    db.pragma_update_and_check(None, "cache_size", format!("-{}", kb), |_| Ok(()))?;
                }
            }
        }
        Ok(client)
    }

    /// Get a [rusqlite::Connection] from the pool.
    pub fn db(&self) -> Result<PooledConnection<SqliteConnectionManager>> {
        self.connection_pool.get().map_err(rusqlite_into_anyhow)
    }

    /// Starts a [Transaction], passing it into the provided `callback`.
    /// On success completion, the transaction is committed.
    pub fn transaction<F>(&self, callback: F) -> Result<Option<Vec<u8>>>
    where
        F: FnOnce(&mut Transaction) -> Result<Option<Vec<u8>>, rusqlite::Error>,
    {
        let mut db = self.db()?;
        let mut tx = db.transaction().map_err(rusqlite_into_anyhow)?;
        let result = callback(&mut tx).map_err(rusqlite_into_anyhow);
        tx.commit().map_err(rusqlite_into_anyhow)?;
        result
    }
}

/// Provides cached queries into table operations, and
/// static methods that generate queries.
///
/// The purpose here is two-fold: While [rusqlite] provides
/// safe parameter interpolation, this doesn't cover e.g. dynamic table names,
/// so we cache them here rather than recreating on every query. Additionally,
/// the static methods consolidate our queries to one place for further inspection.
struct Queries {
    read_stmt: String,
    write_stmt: String,
    delete_stmt: String,
}

impl Queries {
    fn new(table_name: &str) -> Result<Self> {
        Queries::is_sanitized(table_name)?;

        let read_stmt = format!("SELECT VALUE FROM {} WHERE key = $1", table_name);
        let write_stmt = format!(
            "INSERT INTO {} (key, value)
        VALUES($1, $2) 
        ON CONFLICT(key) 
        DO UPDATE SET value=excluded.value",
            table_name
        );
        let delete_stmt = format!("DELETE FROM {} WHERE key = $1", table_name);
        Ok(Queries {
            read_stmt,
            write_stmt,
            delete_stmt,
        })
    }

    pub fn generate_create_table_query(table_name: &str) -> String {
        format!(
            "CREATE TABLE IF NOT EXISTS {} (
                    _id INTEGER PRIMARY KEY,
                    key TEXT NOT NULL UNIQUE,
                    value BLOB
            )",
            table_name
        )
    }

    pub fn read_query(&self) -> &str {
        &self.read_stmt
    }
    pub fn write_query(&self) -> &str {
        &self.write_stmt
    }
    pub fn delete_query(&self) -> &str {
        &self.delete_stmt
    }

    /// Only allow SphereDb table names. Can expand in the future,
    /// but must be sanitized.
    fn is_sanitized(table_name: &str) -> Result<()> {
        match SPHERE_DB_STORE_NAMES.contains(&table_name) {
            true => Ok(()),
            false => Err(anyhow!("Invalid table name.")),
        }
    }
}

#[async_trait]
impl crate::Space for SqliteStorage {
    async fn get_space_usage(&self) -> Result<u64> {
        crate::get_dir_size(&self.client.path).await
    }
}

fn rusqlite_into_anyhow<T: Display>(error: T) -> Error {
    anyhow!(error.to_string())
}
