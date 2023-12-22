use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{store::Store, MemoryStorage, MemoryStore, Storage};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StoreStats {
    pub reads: usize,
    pub writes: usize,
    pub removes: usize,
    pub bytes_read: usize,
    pub bytes_written: usize,
    pub bytes_removed: usize,
    pub flushes: usize,
}

/// This is a store wrapper that tracks I/O. It is inspired by the testing
/// utility originally created for the Forest HAMT implementation. This wrapper
/// is all runtime overhead and should only be used for testing.
#[derive(Debug, Clone)]
pub struct TrackingStore<S: Store> {
    stats: Arc<Mutex<StoreStats>>,
    store: S,
}

impl<S: Store> TrackingStore<S> {
    pub async fn to_stats(&self) -> StoreStats {
        self.stats.lock().await.clone()
    }

    pub fn wrap(store: S) -> Self {
        TrackingStore {
            store,
            stats: Default::default(),
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S: Store> Store for TrackingStore<S> {
    async fn read(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let mut stats = self.stats.lock().await;
        stats.reads += 1;
        let value = self.store.read(key).await?;
        if let Some(bytes) = &value {
            stats.bytes_read += bytes.len();
        }
        Ok(value)
    }

    async fn write(&mut self, key: &[u8], bytes: &[u8]) -> Result<Option<Vec<u8>>> {
        let mut stats = self.stats.lock().await;
        stats.writes += 1;
        stats.bytes_written += bytes.len();
        self.store.write(key, bytes).await
    }

    async fn remove(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let mut stats = self.stats.lock().await;
        stats.removes += 1;
        let value = self.store.remove(key).await?;
        if let Some(bytes) = &value {
            stats.bytes_removed += bytes.len();
        }
        Ok(value)
    }

    async fn flush(&self) -> Result<()> {
        let mut stats = self.stats.lock().await;
        stats.flushes += 1;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct TrackingStorage<S: Storage> {
    storage: S,
}

impl TrackingStorage<MemoryStorage> {
    pub fn wrap(other: MemoryStorage) -> Self {
        TrackingStorage { storage: other }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl Storage for TrackingStorage<MemoryStorage> {
    type BlockStore = TrackingStore<MemoryStore>;

    type KeyValueStore = TrackingStore<MemoryStore>;

    async fn get_block_store(&self, name: &str) -> Result<Self::BlockStore> {
        let block_store = TrackingStore::wrap(self.storage.get_block_store(name).await?);
        Ok(block_store)
    }

    async fn get_key_value_store(&self, name: &str) -> Result<Self::KeyValueStore> {
        let key_value_store = TrackingStore::wrap(self.storage.get_key_value_store(name).await?);
        Ok(key_value_store)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S> crate::EphemeralStorage for TrackingStorage<S>
where
    S: Storage + crate::EphemeralStorage,
{
    type EphemeralStoreType = <S as crate::EphemeralStorage>::EphemeralStoreType;

    async fn get_ephemeral_store(&self) -> Result<crate::EphemeralStore<Self::EphemeralStoreType>> {
        self.storage.get_ephemeral_store().await
    }
}
