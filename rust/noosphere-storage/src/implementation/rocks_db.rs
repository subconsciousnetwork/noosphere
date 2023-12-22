use crate::{
    storage::Storage, store::Store, ConfigurableStorage, PartitionedStore, StorageConfig,
    EPHEMERAL_STORE, SPHERE_DB_STORE_NAMES,
};
use anyhow::{anyhow, Result};
use async_stream::try_stream;
use async_trait::async_trait;
use noosphere_common::ConditionalSend;
use rocksdb::{ColumnFamilyDescriptor, DBWithThreadMode, IteratorMode, Options};
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

        let storage = RocksDbStorage {
            db,
            debug_data: Arc::new((db_path, storage_config)),
        };
        storage.clear_ephemeral()?;
        Ok(storage)
    }

    fn get_store(&self, name: &str) -> Result<RocksDbStore> {
        for store_name in SPHERE_DB_STORE_NAMES {
            if name == *store_name {
                return RocksDbStore::new(self.db.clone(), String::from(name));
            }
        }
        return Err(anyhow!("No such store named {}", name));
    }

    /// Wipes the "ephemeral" column family.
    fn clear_ephemeral(&self) -> Result<()> {
        let ephemeral_store = self.get_store(EPHEMERAL_STORE)?;
        ephemeral_store.remove_range(&[0], &[u8::MAX])
    }
}

#[async_trait]
impl Storage for RocksDbStorage {
    type BlockStore = RocksDbStore;
    type KeyValueStore = RocksDbStore;

    async fn get_block_store(&self, name: &str) -> Result<Self::BlockStore> {
        self.get_store(name)
    }

    async fn get_key_value_store(&self, name: &str) -> Result<Self::KeyValueStore> {
        self.get_store(name)
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

#[async_trait]
impl crate::EphemeralStorage for RocksDbStorage {
    type EphemeralStoreType = EphemeralRocksDbStore;

    async fn get_ephemeral_store(&self) -> Result<crate::EphemeralStore<Self::EphemeralStoreType>> {
        let inner = self.get_store(crate::EPHEMERAL_STORE)?;
        let mapped = crate::PartitionedStore::new(inner);
        Ok(EphemeralRocksDbStore::new(mapped).into())
    }
}

#[async_trait]
impl crate::OpenStorage for RocksDbStorage {
    async fn open<P: AsRef<Path> + ConditionalSend>(path: P) -> Result<Self> {
        RocksDbStorage::new(path)
    }
}

#[async_trait]
impl crate::Space for RocksDbStorage {
    async fn get_space_usage(&self) -> Result<u64> {
        crate::get_dir_size(&self.debug_data.0).await
    }
}

#[derive(Clone)]
pub struct RocksDbStore {
    name: Arc<String>,
    db: Arc<DbInner>,
}

impl RocksDbStore {
    pub(crate) fn new(db: Arc<DbInner>, name: String) -> Result<Self> {
        Ok(RocksDbStore {
            db,
            name: Arc::from(name),
        })
    }

    fn remove_range(&self, from: &[u8], to: &[u8]) -> Result<()> {
        let cf = self.cf_handle()?;
        #[cfg(feature = "rocksdb-multi-thread")]
        let cf = &cf;
        self.db.delete_range_cf(cf, from, to).map_err(|e| e.into())
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
impl crate::IterableStore for RocksDbStore {
    fn get_all_entries(&self) -> std::pin::Pin<Box<crate::IterableStoreStream<'_>>> {
        // handle is not Sync; generate the iterator before
        // async stream work.
        let cf_option = self.db.cf_handle(&self.name);
        let iter = if let Some(cf) = cf_option {
            #[cfg(feature = "rocksdb-multi-thread")]
            let cf = &cf;
            Some(self.db.iterator_cf(cf, IteratorMode::Start))
        } else {
            None
        };
        Box::pin(try_stream! {
            let iter = iter.ok_or_else(|| anyhow!("Could not get cf handle."))?;
            for entry in iter {
                let (key, value) = entry?;
                yield (Vec::from(key.as_ref()), Vec::from(value.as_ref()));
            }
        })
    }
}

/// A [RocksDbStore] that does not persist data after dropping.
/// Can be created from [IndexedDbStorage::get_ephemeral_store].
#[derive(Clone)]
pub struct EphemeralRocksDbStore {
    store: PartitionedStore<RocksDbStore>,
}

impl EphemeralRocksDbStore {
    pub(crate) fn new(store: PartitionedStore<RocksDbStore>) -> Self {
        EphemeralRocksDbStore { store }
    }
}

#[async_trait]
impl Store for EphemeralRocksDbStore {
    async fn read(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        self.store.read(key).await
    }

    async fn write(&mut self, key: &[u8], bytes: &[u8]) -> Result<Option<Vec<u8>>> {
        self.store.write(key, bytes).await
    }

    async fn remove(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        self.store.remove(key).await
    }

    async fn flush(&self) -> Result<()> {
        self.store.flush().await
    }
}

#[async_trait]
impl crate::Disposable for EphemeralRocksDbStore {
    async fn dispose(&mut self) -> Result<()> {
        let (start_key, end_key) = self.store.get_key_range();
        self.store.inner().remove_range(start_key, end_key)
    }
}

impl crate::IterableStore for EphemeralRocksDbStore {
    fn get_all_entries(&self) -> std::pin::Pin<Box<crate::IterableStoreStream<'_>>> {
        self.store.get_all_entries()
    }
}
