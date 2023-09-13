use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{store::Store, MemoryStorage, MemoryStore, Storage};

/// Stats derived from a [TrackingStore].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StoreStats {
    /// Number of reads performed.
    pub reads: usize,
    /// Number of writes performed.
    pub writes: usize,
    /// Number of removes performed.
    pub removes: usize,
    /// Number of bytes read.
    pub bytes_read: usize,
    /// Number of bytes written.
    pub bytes_written: usize,
    /// Number of bytes removed.
    pub bytes_removed: usize,
    /// Number of flushes.
    pub flushes: usize,
}

/// A [Store] implementation for [TrackingStorage].
#[derive(Debug, Clone)]
pub struct TrackingStore<S: Store> {
    stats: Arc<Mutex<StoreStats>>,
    store: S,
}

impl<S: Store> TrackingStore<S> {
    /// Returns the current [StoreStats] snapshot.
    pub async fn to_stats(&self) -> StoreStats {
        self.stats.lock().await.clone()
    }

    /// Create a new [TrackingStore] wrapping a [Store].
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

/// A [Storage] wrapper that tracks I/O.
///
/// It is inspired by the testing utility originally created for
/// the Forest HAMT implementation. This wrapper is all runtime
/// overhead and should only be used for testing.
#[derive(Clone, Debug)]
pub struct TrackingStorage<S: Storage> {
    storage: S,
}

impl TrackingStorage<MemoryStorage> {
    /// Create a new [TrackingStorage] wrapping a [MemoryStorage].
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
