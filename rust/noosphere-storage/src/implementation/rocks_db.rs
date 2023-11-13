use crate::{
    storage::Storage, store::Store, ConfigurableStorage, StorageConfig, SPHERE_DB_STORE_NAMES,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use noosphere_common::ConditionalSend;
use rocksdb::{ColumnFamilyDescriptor, DBWithThreadMode, Options};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

#[cfg(not(feature = "rocksdb-multi-thread"))]
type DbInner = DBWithThreadMode<rocksdb::SingleThreaded>;
#[cfg(not(feature = "rocksdb-multi-thread"))]
type ColumnType<'a> = &'a rocksdb::ColumnFamily;
#[cfg(feature = "rocksdb-multi-thread")]
type DbInner = DBWithThreadMode<rocksdb::MultiThreaded>;
#[cfg(feature = "rocksdb-multi-thread")]
type ColumnType<'a> = Arc<rocksdb::BoundColumnFamily<'a>>;

/// A RocksDB implementation of [Storage].
///
/// Caveats:
/// * Values are limited to 4GB(?) [https://github.com/facebook/rocksdb/wiki/Basic-Operations#reads]
/// TODO(#631): Further improvements to the implementation.
#[derive(Clone)]
pub struct RocksDbStorage {
    db: Arc<DbInner>,
    debug_data: Arc<(PathBuf, StorageConfig)>,
}

impl RocksDbStorage {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::with_config(path, StorageConfig::default())
    }

    pub fn with_config<P: AsRef<Path>>(path: P, storage_config: StorageConfig) -> Result<Self> {
        std::fs::create_dir_all(path.as_ref())?;
        let db_path = path.as_ref().canonicalize()?;
        let db = {
            let mut cfs: Vec<ColumnFamilyDescriptor> =
                Vec::with_capacity(SPHERE_DB_STORE_NAMES.len());

            for store_name in SPHERE_DB_STORE_NAMES {
                // https://github.com/facebook/rocksdb/wiki/RocksDB-Tuning-Guide
                let cf_opts = Options::default();
                cfs.push(ColumnFamilyDescriptor::new(*store_name, cf_opts));
            }

            let mut db_opts = Options::default();
            db_opts.create_if_missing(true);
            db_opts.create_missing_column_families(true);

            if let Some(memory_cache_limit) = storage_config.memory_cache_limit {
                // Amount of data to build up in memtables across all column families before writing to disk.
                db_opts.set_db_write_buffer_size(memory_cache_limit);
            }

            Arc::new(DbInner::open_cf_descriptors(&db_opts, path, cfs)?)
        };

        Ok(RocksDbStorage {
            db,
            debug_data: Arc::new((db_path, storage_config)),
        })
    }

    async fn get_store(&self, name: &str) -> Result<RocksDbStore> {
        if SPHERE_DB_STORE_NAMES
            .iter()
            .find(|val| **val == name)
            .is_none()
        {
            return Err(anyhow!("No such store named {}", name));
        }

        RocksDbStore::new(self.db.clone(), name.to_owned())
    }
}

#[async_trait]
impl Storage for RocksDbStorage {
    type BlockStore = RocksDbStore;
    type KeyValueStore = RocksDbStore;

    async fn get_block_store(&self, name: &str) -> Result<Self::BlockStore> {
        self.get_store(name).await
    }

    async fn get_key_value_store(&self, name: &str) -> Result<Self::KeyValueStore> {
        self.get_store(name).await
    }
}

#[async_trait]
impl ConfigurableStorage for RocksDbStorage {
    async fn open_with_config<P: AsRef<Path> + ConditionalSend>(
        path: P,
        storage_config: StorageConfig,
    ) -> Result<Self> {
        Self::with_config(path, storage_config)
    }
}

impl std::fmt::Debug for RocksDbStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RocksDbStorage")
            .field("path", &self.debug_data.0)
            .field("config", &self.debug_data.1)
            .finish()
    }
}

#[derive(Clone)]
pub struct RocksDbStore {
    name: String,
    db: Arc<DbInner>,
}

impl RocksDbStore {
    pub fn new(db: Arc<DbInner>, name: String) -> Result<Self> {
        Ok(RocksDbStore { db, name })
    }

    /// Returns the column family handle. Unfortunately generated on every call
    /// due to not being `Sync`, potentially `unsafe` alternatives:
    /// https://github.com/rust-rocksdb/rust-rocksdb/issues/407
    /// TODO(#631): Further improvements to the implementation.
    fn cf_handle<'a>(&'a self) -> Result<ColumnType> {
        self.db
            .cf_handle(&self.name)
            .ok_or_else(move || anyhow!("Could not open handle for {}", self.name))
    }
}

#[async_trait]
impl Store for RocksDbStore {
    async fn read(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let cf = self.cf_handle()?;
        #[cfg(feature = "rocksdb-multi-thread")]
        let cf = &cf;
        Ok(self.db.get_cf(cf, key)?)
    }

    async fn write(&mut self, key: &[u8], bytes: &[u8]) -> Result<Option<Vec<u8>>> {
        let cf = self.cf_handle()?;
        #[cfg(feature = "rocksdb-multi-thread")]
        let cf = &cf;
        let old_bytes = self.db.get_cf(cf, key)?;
        self.db.put_cf(cf, key, bytes)?;
        Ok(old_bytes)
    }

    async fn remove(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let cf = self.cf_handle()?;
        #[cfg(feature = "rocksdb-multi-thread")]
        let cf = &cf;
        let old_bytes = self.db.get_cf(cf, key)?;
        self.db.delete_cf(cf, key)?;
        Ok(old_bytes)
    }

    async fn flush(&self) -> Result<()> {
        // With the use of WAL, we do not want to actively flush on every sync,
        // and instead allow RocksDB to determine when to flush to OS.
        Ok(())
    }
}

#[async_trait]
impl crate::Space for RocksDbStorage {
    async fn get_space_usage(&self) -> Result<u64> {
        crate::get_dir_size(&self.debug_data.0).await
    }
}
