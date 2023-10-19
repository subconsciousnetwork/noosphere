use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::store::Store;
use crate::StorageConfig;
use crate::{storage::Storage, ConfigurableStorage};

use anyhow::Result;
use async_trait::async_trait;
use noosphere_common::ConditionalSend;
use sled::{Db, Tree};

#[derive(Clone)]
pub struct SledStorage {
    db: Db,
    debug_data: Arc<(PathBuf, StorageConfig)>,
}

impl SledStorage {
    /// Open or create a database at directory `path`.
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::with_config(path, StorageConfig::default())
    }

    pub fn with_config<P: AsRef<Path>>(path: P, config: StorageConfig) -> Result<Self> {
        std::fs::create_dir_all(path.as_ref())?;
        let db_path = path.as_ref().canonicalize()?;

        let mut sled_config = sled::Config::default();
        sled_config = sled_config.path(&db_path);
        if let Some(memory_cache_limit) = config.memory_cache_limit {
            // Maximum size in bytes for the system page cache. (default: 1GB)
            sled_config = sled_config.cache_capacity(memory_cache_limit.try_into()?);
        }

        let db = sled_config.open()?;
        let debug_data = Arc::new((db_path, config));
        Ok(SledStorage { db, debug_data })
    }

    async fn get_store(&self, name: &str) -> Result<SledStore> {
        Ok(SledStore::new(&self.db.open_tree(name)?))
    }
}

#[async_trait]
impl Storage for SledStorage {
    type BlockStore = SledStore;

    type KeyValueStore = SledStore;

    async fn get_block_store(&self, name: &str) -> Result<Self::BlockStore> {
        self.get_store(name).await
    }

    async fn get_key_value_store(&self, name: &str) -> Result<Self::KeyValueStore> {
        self.get_store(name).await
    }
}

#[async_trait]
impl ConfigurableStorage for SledStorage {
    async fn open_with_config<P: AsRef<Path> + ConditionalSend>(
        path: P,
        config: StorageConfig,
    ) -> Result<Self> {
        SledStorage::with_config(path, config)
    }
}

impl std::fmt::Debug for SledStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SledStorage")
            .field("path", &self.debug_data.0)
            .field("config", &self.debug_data.1)
            .finish()
    }
}

#[derive(Clone)]
pub struct SledStore {
    db: Tree,
}

impl SledStore {
    pub fn new(db: &Tree) -> Self {
        SledStore { db: db.clone() }
    }
}

#[async_trait]
impl Store for SledStore {
    async fn read(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        Ok(self.db.get(key)?.map(|entry| entry.to_vec()))
    }

    async fn write(&mut self, key: &[u8], bytes: &[u8]) -> Result<Option<Vec<u8>>> {
        let old_bytes = self
            .db
            .insert(key, bytes)?
            .map(|old_entry| old_entry.to_vec());
        Ok(old_bytes)
    }

    /// Remove a value given a CID
    async fn remove(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        Ok(self
            .db
            .remove(key)
            .map(|maybe_entry| maybe_entry.map(|entry| entry.to_vec()))?)
    }

    /// Flushes pending writes if there are any
    async fn flush(&self) -> Result<()> {
        // `flush_async()` can deadlock when simultaneous calls are performed.
        // This occurs often in tests and fixed in `sled`'s main branch,
        // but no cargo release since 2021.
        // https://github.com/spacejam/sled/issues/1308
        self.db.flush()?;
        Ok(())
    }
}

impl Drop for SledStorage {
    fn drop(&mut self) {
        let _ = self.db.flush();
    }
}

#[async_trait]
impl crate::Space for SledStorage {
    async fn get_space_usage(&self) -> Result<u64> {
        self.db.size_on_disk().map_err(|e| e.into())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{KeyValueStore, Store};
    #[tokio::test]
    async fn it_can_be_closed_and_reopened_raw() -> Result<()> {
        let tempdir = tempfile::TempDir::new()?;
        let storage_path = tempdir.path().to_owned();

        let db = sled::open(&storage_path)?;
        db.insert(b"foo", b"bar")?;
        db.flush()?;
        drop(db);

        for _ in 0..50 {
            let db = sled::open(&storage_path)?;
            db.insert(b"foo", b"bar")?;
            db.flush()?;
        }
        Ok(())
    }

    #[tokio::test]
    async fn it_can_be_closed_and_reopened_multithread() -> Result<()> {
        let tempdir = tempfile::TempDir::new()?;
        let storage_path = tempdir.path().to_owned();

        for _ in 0..20 {
            let path = storage_path.clone();
            tokio::task::spawn(async move {
                let storage = SledStorage::new(&path)?;
                let mut store = storage.get_key_value_store("links").await?;
                store.set_key("foo-1", "123").await?;
                Store::flush(&store).await?;
                Result::<(), anyhow::Error>::Ok(())
            })
            .await??;
        }
        Ok(())
    }

    #[tokio::test]
    async fn it_can_be_closed_and_reopened() -> Result<()> {
        let tempdir = tempfile::TempDir::new()?;
        let storage_path = tempdir.path().to_owned();

        for _ in 0..20 {
            let _storage = SledStorage::new(&storage_path)?;
        }

        for _ in 0..20 {
            let storage = SledStorage::new(&storage_path)?;
            let mut store = storage.get_key_value_store("links").await?;
            store.set_key("foo-1", "123").await?;
            Store::flush(&store).await?;
        }
        Ok(())
    }
}
