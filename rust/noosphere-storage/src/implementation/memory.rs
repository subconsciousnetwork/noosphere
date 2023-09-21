use anyhow::{anyhow, Result};
use async_trait::async_trait;
use cid::Cid;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

use crate::storage::Storage;
use crate::store::Store;

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait StoreContainsCid {
    async fn contains_cid(&self, cid: &Cid) -> Result<bool>;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S: Store> StoreContainsCid for S {
    async fn contains_cid(&self, cid: &Cid) -> Result<bool> {
        Ok(self.read(&cid.to_bytes()).await?.is_some())
    }
}

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

#[derive(Clone, Default, Debug)]
pub struct MemoryStore {
    pub entries: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
}

impl MemoryStore {
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

    pub async fn expect_replica_in<S: Store>(&self, other: &S) -> Result<()> {
        let cids = self.get_stored_cids().await;
        let mut missing = Vec::new();

        for cid in cids {
            trace!("Checking for {}", cid);

            if !other.contains_cid(&cid).await? {
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
impl crate::EphemeralStorage for MemoryStorage {
    type EphemeralStoreType = MemoryStore;

    async fn get_ephemeral_store(&self) -> Result<crate::EphemeralStore<Self::EphemeralStoreType>> {
        Ok(MemoryStore::default().into())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl crate::Disposable for MemoryStore {
    async fn dispose(&mut self) -> Result<()> {
        Ok(())
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
