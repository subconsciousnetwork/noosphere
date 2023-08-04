use std::path::PathBuf;

use crate::storage::Storage;
use crate::store::Store;

use anyhow::Result;
use async_trait::async_trait;
use sled::{Db, Tree};

pub enum NativeStorageInit {
    Path(PathBuf),
    Db(Db),
}

#[derive(Clone, Debug)]
pub struct NativeStorage {
    db: Db,
}

impl NativeStorage {
    pub fn new(init: NativeStorageInit) -> Result<Self> {
        let db: Db = match init {
            NativeStorageInit::Path(path) => {
                std::fs::create_dir_all(&path)?;
                sled::open(path)?
            }
            NativeStorageInit::Db(db) => db,
        };

        Ok(NativeStorage { db })
    }

    async fn get_store(&self, name: &str) -> Result<NativeStore> {
        Ok(NativeStore::new(&self.db.open_tree(name)?))
    }
}

#[async_trait]
impl Storage for NativeStorage {
    type BlockStore = NativeStore;

    type KeyValueStore = NativeStore;

    async fn get_block_store(&self, name: &str) -> Result<Self::BlockStore> {
        self.get_store(name).await
    }

    async fn get_key_value_store(&self, name: &str) -> Result<Self::KeyValueStore> {
        self.get_store(name).await
    }
}

#[derive(Clone)]
pub struct NativeStore {
    db: Tree,
}

impl NativeStore {
    pub fn new(db: &Tree) -> Self {
        NativeStore { db: db.clone() }
    }
}

#[async_trait]
impl Store for NativeStore {
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
