use anyhow::Result;
use async_std::sync::Mutex;
use async_trait::async_trait;
use std::sync::Arc;

use crate::interface::Store;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StoreStats {
    pub reads: usize,
    pub writes: usize,
    pub removes: usize,
    pub bytes_read: usize,
    pub bytes_written: usize,
    pub bytes_removed: usize,
}

/// This is a store wrapper that tracks I/O. It is inspired by the testing
/// utility originally created for the Forest HAMT implementation. This wrapper
/// is all runtime overhead and should only be used for testing.
#[derive(Debug, Clone)]
pub struct TrackingStore<Storage: Store> {
    stats: Arc<Mutex<StoreStats>>,
    store: Storage,
}

impl<Storage: Store> TrackingStore<Storage> {
    pub async fn to_stats(&self) -> StoreStats {
        self.stats.lock().await.clone()
    }

    pub fn wrap(store: Storage) -> Self {
        TrackingStore {
            store,
            stats: Default::default(),
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<Storage: Store> Store for TrackingStore<Storage> {
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
}
