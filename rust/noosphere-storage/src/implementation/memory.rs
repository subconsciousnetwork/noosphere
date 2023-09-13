use anyhow::{anyhow, Result};
use async_stream::try_stream;
use async_trait::async_trait;
use cid::Cid;
use noosphere_common::ConditionalSend;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

use crate::storage::Storage;
use crate::store::Store;

async fn contains_cid<S: Store>(store: &S, cid: &Cid) -> Result<bool> {
    Ok(store.read(&cid.to_bytes()).await?.is_some())
}

/// A memory-backed [Storage] implementation.
///
/// Useful for small, short-lived storages and testing.
#[derive(Default, Clone, Debug)]
pub struct MemoryStorage {
    stores: Arc<Mutex<HashMap<String, MemoryStore>>>,
}

impl MemoryStorage {
    async fn get_store(&self, name: &str) -> Result<MemoryStore> {
        let mut stores = self.stores.lock().await;

        if !stores.contains_key(name) {
            stores.insert(name.to_string(), Default::default());
        }

        stores
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow!("Failed to initialize {} store", name))
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl Storage for MemoryStorage {
    type BlockStore = MemoryStore;

    type KeyValueStore = MemoryStore;

    async fn get_block_store(&self, name: &str) -> Result<Self::BlockStore> {
        self.get_store(name).await
    }

    async fn get_key_value_store(&self, name: &str) -> Result<Self::KeyValueStore> {
        self.get_store(name).await
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl crate::ops::OpenStorage for MemoryStorage {
    async fn open<P: AsRef<std::path::Path> + ConditionalSend>(_: P) -> Result<Self> {
        Ok(MemoryStorage::default())
    }
}

/// [Store] implementation for [MemoryStorage].
#[derive(Clone, Default, Debug)]
pub struct MemoryStore {
    /// Underlying key-value store.
    pub entries: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
}

impl MemoryStore {
    /// Return all [Cid] keys from the store.
    pub async fn get_stored_cids(&self) -> Vec<Cid> {
        self.entries
            .lock()
            .await
            .keys()
            .filter_map(|bytes| match Cid::try_from(bytes.as_slice()) {
                Ok(cid) => Some(cid),
                _ => None,
            })
            .collect()
    }

    /// Returns `Ok` if all entries in this store are replicated by `other`.
    pub async fn expect_replica_in<S: Store>(&self, other: &S) -> Result<()> {
        let cids = self.get_stored_cids().await;
        let mut missing = Vec::new();

        for cid in cids {
            trace!("Checking for {}", cid);

            if !contains_cid(other, &cid).await? {
                trace!("Not found!");
                missing.push(cid);
            }
        }

        if !missing.is_empty() {
            return Err(anyhow!(
                "Expected replica, but the following CIDs are missing: {:#?}",
                missing
                    .into_iter()
                    .map(|cid| format!("{cid}"))
                    .collect::<Vec<String>>()
            ));
        }

        Ok(())
    }

    /// Clones this store, sharing the underlying data.
    pub async fn fork(&self) -> Self {
        MemoryStore {
            entries: Arc::new(Mutex::new(self.entries.lock().await.clone())),
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl Store for MemoryStore {
    async fn read(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let dags = self.entries.lock().await;
        Ok(dags.get(key).cloned())
    }

    async fn write(&mut self, key: &[u8], bytes: &[u8]) -> Result<Option<Vec<u8>>> {
        let mut dags = self.entries.lock().await;
        let old_value = dags.get(key).cloned();

        dags.insert(key.to_vec(), bytes.to_vec());

        Ok(old_value)
    }

    async fn remove(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let mut dags = self.entries.lock().await;
        Ok(dags.remove(key))
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl crate::IterableStore for MemoryStore {
    fn get_all_entries(&self) -> std::pin::Pin<Box<crate::IterableStoreStream<'_>>> {
        Box::pin(try_stream! {
            let dags = self.entries.lock().await;
            for key in dags.keys() {
                yield (key.to_owned(), dags.get(key).cloned());
            }
        })
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl crate::Space for MemoryStorage {
    async fn get_space_usage(&self) -> Result<u64> {
        let mut size = 0;
        for (_, store) in self.stores.lock().await.iter() {
            for (key, entry) in store.entries.lock().await.iter() {
                size += key.len() as u64;
                size += entry.len() as u64;
            }
        }
        Ok(size)
    }
}
