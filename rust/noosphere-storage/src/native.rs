use std::path::PathBuf;

use crate::interface::{StorageProvider, Store};
use anyhow::Result;
use async_trait::async_trait;
use sled::{Db, Tree};

pub enum NativeStorageInit {
    Path(PathBuf),
    Db(Db),
}

#[derive(Clone)]
pub struct NativeStorageProvider {
    db: Db,
}

impl NativeStorageProvider {
    pub fn new(init: NativeStorageInit) -> Result<Self> {
        let db: Db = match init {
            NativeStorageInit::Path(path) => sled::open(path)?,
            NativeStorageInit::Db(db) => db,
        };

        Ok(NativeStorageProvider { db })
    }
}

#[async_trait]
impl StorageProvider<NativeStore> for NativeStorageProvider {
    async fn get_store(&self, name: &str) -> Result<NativeStore> {
        Ok(NativeStore::new(&self.db.open_tree(name)?))
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
    // impl Store for NativeStore {
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

    async fn contains(&self, key: &[u8]) -> Result<bool> {
        Ok(self.db.contains_key(key)?)
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
        self.db.flush_async().await?;
        Ok(())
    }
}
