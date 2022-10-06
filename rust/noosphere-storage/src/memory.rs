use anyhow::{anyhow, Result};
use async_std::sync::Mutex;
use async_trait::async_trait;
use cid::Cid;
use std::{collections::HashMap, sync::Arc};

use crate::interface::{StorageProvider, Store};

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

#[derive(Default)]
pub struct MemoryStorageProvider {
    stores: Arc<Mutex<HashMap<String, MemoryStore>>>,
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl StorageProvider<MemoryStore> for MemoryStorageProvider {
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
                    .map(|cid| format!("{}", cid))
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
