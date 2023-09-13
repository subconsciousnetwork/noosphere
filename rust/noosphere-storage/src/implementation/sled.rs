use std::path::{Path, PathBuf};

use crate::storage::Storage;
use crate::store::Store;

use anyhow::Result;
use async_stream::try_stream;
use async_trait::async_trait;
use noosphere_common::ConditionalSend;
use sled::{Db, Tree};

/// A [Sled](https://github.com/spacejam/sled) [Storage] implementation.
#[derive(Clone, Debug)]
pub struct SledStorage {
    db: Db,
    #[allow(unused)]
    path: PathBuf,
}

impl SledStorage {
    /// Open or create a database at directory `path`.
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        std::fs::create_dir_all(path.as_ref())?;
        let db_path = path.as_ref().canonicalize()?;
        let db = sled::open(&db_path)?;

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

#[async_trait]
impl crate::FsBackedStorage for SledStorage {}

#[async_trait]
impl crate::OpenStorage for SledStorage {
    async fn open<P: AsRef<Path> + ConditionalSend>(path: P) -> Result<Self> {
        SledStorage::new(path)
    }
}

#[async_trait]
impl crate::Space for SledStorage {
    async fn get_space_usage(&self) -> Result<u64> {
        self.db.size_on_disk().map_err(|e| e.into())
    }
}

/// [Store] implementation for [SledStorage].
#[derive(Clone)]
pub struct SledStore {
    db: Tree,
}

impl SledStore {
    pub(crate) fn new(db: &Tree) -> Self {
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

impl crate::IterableStore for SledStore {
    fn get_all_entries(&self) -> std::pin::Pin<Box<crate::IterableStoreStream<'_>>> {
        Box::pin(try_stream! {
            for entry in self.db.iter() {
                let (key, value) = entry?;
                yield (Vec::from(key.as_ref()), Some(Vec::from(value.as_ref())));
            }
        })
    }
}
