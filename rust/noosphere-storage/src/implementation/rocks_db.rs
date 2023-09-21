use crate::{
    storage::Storage, store::Store, PartitionedStore, EPHEMERAL_STORE, SPHERE_DB_STORE_NAMES,
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
#[derive(Clone, Debug)]
pub struct RocksDbStorage {
    db: Arc<DbInner>,
    #[allow(unused)]
    path: PathBuf,
}

impl RocksDbStorage {
    pub async fn new<P: AsRef<Path> + ConditionalSend>(path: P) -> Result<Self> {
        std::fs::create_dir_all(path.as_ref())?;
        let canonicalized = path.as_ref().canonicalize()?;
        let db = Arc::new(RocksDbStorage::init_db(canonicalized.clone())?);
        let storage = RocksDbStorage {
            db,
            path: canonicalized,
        };
        storage.clear_ephemeral().await?;
        Ok(storage)
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

    /// Configures a databasea at `path` and initializes the expected configurations.
    fn init_db<P: AsRef<Path>>(path: P) -> Result<DbInner> {
        let mut cfs: Vec<ColumnFamilyDescriptor> = Vec::with_capacity(SPHERE_DB_STORE_NAMES.len());

        for store_name in SPHERE_DB_STORE_NAMES {
            // https://github.com/facebook/rocksdb/wiki/RocksDB-Tuning-Guide
            let cf_opts = Options::default();
            cfs.push(ColumnFamilyDescriptor::new(*store_name, cf_opts));
        }

        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);

        Ok(DbInner::open_cf_descriptors(&db_opts, path, cfs)?)
    }

    /// Wipes the "ephemeral" column family.
    async fn clear_ephemeral(&self) -> Result<()> {
        let ephemeral_store = self.get_store(EPHEMERAL_STORE).await?;
        ephemeral_store.remove_range(&[0], &[u8::MAX]).await
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
impl crate::EphemeralStorage for RocksDbStorage {
    type EphemeralStoreType = EphemeralRocksDbStore;

    async fn get_ephemeral_store(&self) -> Result<crate::EphemeralStore<Self::EphemeralStoreType>> {
        let inner = self.get_store(crate::EPHEMERAL_STORE).await?;
        let mapped = crate::PartitionedStore::new(inner);
        Ok(EphemeralRocksDbStore::new(mapped).into())
    }
}

#[async_trait]
impl crate::OpenStorage for RocksDbStorage {
    async fn open<P: AsRef<Path> + ConditionalSend>(path: P) -> Result<Self> {
        RocksDbStorage::new(path).await
    }
}

#[async_trait]
impl crate::Space for RocksDbStorage {
    async fn get_space_usage(&self) -> Result<u64> {
        crate::get_dir_size(&self.path).await
    }
}

#[derive(Clone)]
pub struct RocksDbStore {
    name: String,
    db: Arc<DbInner>,
}

impl RocksDbStore {
    pub(crate) fn new(db: Arc<DbInner>, name: String) -> Result<Self> {
        Ok(RocksDbStore { db, name })
    }

    async fn remove_range(&self, from: &[u8], to: &[u8]) -> Result<()> {
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
        self.store.inner().remove_range(start_key, end_key).await
    }
}

impl crate::IterableStore for EphemeralRocksDbStore {
    fn get_all_entries(&self) -> std::pin::Pin<Box<crate::IterableStoreStream<'_>>> {
        self.store.get_all_entries()
    }
}
