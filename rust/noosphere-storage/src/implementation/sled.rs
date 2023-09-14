use std::path::PathBuf;

use crate::storage::Storage;
use crate::store::Store;

use anyhow::Result;
use async_trait::async_trait;
use sled::{Db, Tree};

pub enum SledStorageInit {
    Path(PathBuf),
    Db(Db),
}

#[derive(Clone, Debug)]
pub struct SledStorage {
    db: Db,
    #[allow(unused)]
    path: Option<PathBuf>,
}

impl SledStorage {
    pub fn new(init: SledStorageInit) -> Result<Self> {
        let mut db_path = None;
        let db: Db = match init {
            SledStorageInit::Path(path) => {
                std::fs::create_dir_all(&path)?;
                db_path = Some(path.clone().canonicalize()?);
                sled::open(path)?
            }
            SledStorageInit::Db(db) => db,
        };

        Ok(SledStorage { db, path: db_path })
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
