use std::path::{Path, PathBuf};

use crate::storage::Storage;
use crate::store::Store;

use anyhow::{anyhow, Result};
use async_stream::try_stream;
use async_trait::async_trait;
use noosphere_common::ConditionalSend;
use sled::{Db, Tree};

pub(crate) const EPHEMERAL_SLED_PREFIX: &str = "EPHEMERAL-SLED-STORAGE";

pub enum SledStorageInit {
    Path(PathBuf),
    Db(Db),
}

#[derive(Clone, Debug)]
pub struct SledStorage {
    db: Db,
    _path: PathBuf,
}

impl SledStorage {
    /// Open or create a database at directory `path`.
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        std::fs::create_dir_all(path.as_ref())?;
        let db_path = path.as_ref().canonicalize()?;
        let db = sled::open(&db_path)?;

        let storage = SledStorage { db, _path: db_path };
        storage.clear_ephemeral()?;
        Ok(storage)
    }

    async fn get_store(&self, name: &str) -> Result<SledStore> {
        Ok(SledStore::new(&self.db.open_tree(name)?))
    }

    #[cfg(test)]
    #[allow(unused)]
    pub(crate) fn inner(&self) -> &Db {
        &self.db
    }

    /// Wipes all "ephemeral" trees.
    fn clear_ephemeral(&self) -> Result<()> {
        for name in self.db.tree_names() {
            let tree_name = String::from_utf8(Vec::from(name.as_ref()))?;
            if tree_name.starts_with(EPHEMERAL_SLED_PREFIX) {
                match self.db.drop_tree(tree_name.as_bytes())? {
                    true => continue,
                    false => {
                        warn!("Could not drop ephemeral tree {}", tree_name);
                        continue;
                    }
                }
            }
        }
        Ok(())
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

impl Drop for SledStorage {
    fn drop(&mut self) {
        let _ = self.db.flush();
    }
}

#[async_trait]
impl crate::EphemeralStorage for SledStorage {
    type EphemeralStoreType = EphemeralSledStore;

    async fn get_ephemeral_store(&self) -> Result<crate::EphemeralStore<Self::EphemeralStoreType>> {
        Ok(EphemeralSledStore::new(self.db.clone())?.into())
    }
}

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

/// A [SledStore] that does not persist data after dropping.
/// Can be created from [SledStorage]'s [crate::EphemeralStorage] implementation.
#[derive(Clone)]
pub struct EphemeralSledStore {
    db: Db,
    name: String,
    store: SledStore,
}

impl EphemeralSledStore {
    pub(crate) fn new(db: Db) -> Result<Self> {
        let name = format!("{}-{}", EPHEMERAL_SLED_PREFIX, rand::random::<u32>());
        let store = SledStore::new(&db.open_tree(&name)?);
        Ok(EphemeralSledStore { db, store, name })
    }
}

#[async_trait]
impl Store for EphemeralSledStore {
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

impl crate::IterableStore for SledStore {
    fn get_all_entries(&self) -> std::pin::Pin<Box<crate::IterableStoreStream<'_>>> {
        Box::pin(try_stream! {
            for entry in self.db.iter() {
                let (key, value) = entry?;
                yield (Vec::from(key.as_ref()), Vec::from(value.as_ref()));
            }
        })
    }
}

#[async_trait]
impl crate::Disposable for EphemeralSledStore {
    async fn dispose(&mut self) -> Result<()> {
        self.db.drop_tree(&self.name).map_or_else(
            |e| Err(e.into()),
            |bool_state| match bool_state {
                true => Ok(()),
                false => Err(anyhow!("Could not clear temporary tree.")),
            },
        )
    }
}

impl crate::IterableStore for EphemeralSledStore {
    fn get_all_entries(&self) -> std::pin::Pin<Box<crate::IterableStoreStream<'_>>> {
        self.store.get_all_entries()
    }
}
