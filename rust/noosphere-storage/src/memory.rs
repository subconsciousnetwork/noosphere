use anyhow::Result;
use async_std::sync::Mutex;
use async_trait::async_trait;
use cid::Cid;
use std::{collections::HashMap, sync::Arc};

use crate::interface::Store;

#[derive(Clone, Default, Debug)]
pub struct MemoryStore {
    dags: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
}

impl MemoryStore {
    pub async fn get_stored_cids(&self) -> Vec<Cid> {
        self.dags
            .lock()
            .await
            .keys()
            .filter_map(|bytes| match Cid::try_from(bytes.as_slice()) {
                Ok(cid) => Some(cid),
                _ => None,
            })
            .collect()
    }

    pub async fn fork(&self) -> Self {
        MemoryStore {
            dags: Arc::new(Mutex::new(self.dags.lock().await.clone())),
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl Store for MemoryStore {
    async fn read(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let dags = self.dags.lock().await;
        Ok(dags.get(key).cloned())
    }

    async fn write(&mut self, key: &[u8], bytes: &[u8]) -> Result<Option<Vec<u8>>> {
        let mut dags = self.dags.lock().await;
        let old_value = dags.get(key).cloned();

        dags.insert(key.to_vec(), bytes.to_vec());

        Ok(old_value)
    }

    async fn contains(&self, key: &[u8]) -> Result<bool> {
        let dags = self.dags.lock().await;
        Ok(dags.contains_key(key))
    }

    async fn remove(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let mut dags = self.dags.lock().await;
        Ok(dags.remove(key))
    }
}
