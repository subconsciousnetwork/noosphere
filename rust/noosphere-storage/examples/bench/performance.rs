use anyhow::{Error, Result};
use async_trait::async_trait;
use instant::{Duration, Instant};
use noosphere_storage::{EphemeralStorage, EphemeralStore, Space, Storage, Store};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Default)]
pub struct PerformanceStats {
    pub reads: PerformanceAnalysis,
    pub writes: PerformanceAnalysis,
    pub removes: PerformanceAnalysis,
    pub flushes: PerformanceAnalysis,
    pub logical_bytes_stored: u64,
    pub physical_bytes_stored: u64,
}

#[derive(Debug, Clone, Default)]
pub struct PerformanceAnalysis {
    pub mean: f64,
    pub count: usize,
}

impl TryFrom<Vec<Duration>> for PerformanceAnalysis {
    type Error = Error;
    fn try_from(value: Vec<Duration>) -> Result<Self> {
        let mut durations_us: Vec<_> = value
            .into_iter()
            .map(|d| TryInto::<u32>::try_into(d.as_micros()))
            .collect::<Result<Vec<u32>, std::num::TryFromIntError>>()?;
        durations_us.sort();
        let count = durations_us.len();
        let mean = if count == 0 {
            0.0
        } else {
            durations_us.iter().sum::<u32>() as f64 / count as f64
        };
        Ok(Self { count, mean })
    }
}

impl TryFrom<InternalStoreStats> for PerformanceStats {
    type Error = Error;
    fn try_from(value: InternalStoreStats) -> Result<Self> {
        Ok(PerformanceStats {
            reads: value.reads.try_into()?,
            writes: value.writes.try_into()?,
            removes: value.removes.try_into()?,
            flushes: value.flushes.try_into()?,
            logical_bytes_stored: value.logical_bytes_stored,
            ..Default::default()
        })
    }
}

#[derive(Debug, Default)]
struct InternalStoreStats {
    pub reads: Vec<Duration>,
    pub writes: Vec<Duration>,
    pub removes: Vec<Duration>,
    pub flushes: Vec<Duration>,
    pub logical_bytes_stored: u64,
}

/// A wrapper for [Storage] types that tracks performance
/// of various operations.
/// If [Storage] is also [Space], [PerformanceStorage::as_stats] can be
/// called to get performance data as [PerformanceStats].
#[derive(Clone, Debug)]
pub struct PerformanceStorage<S: Storage> {
    storage: S,
    stats: Arc<Mutex<HashMap<String, Arc<Mutex<InternalStoreStats>>>>>,
}

impl<S> PerformanceStorage<S>
where
    S: Storage,
    S::KeyValueStore: Store,
    S::BlockStore: Store,
{
    pub fn new(other: S) -> Self {
        PerformanceStorage {
            storage: other,
            stats: Arc::new(Mutex::new(HashMap::default())),
        }
    }

    async fn get_store_stats(&self, name: &str) -> Arc<Mutex<InternalStoreStats>> {
        let mut storage_stats = self.stats.lock().await;
        let store_name = name.to_owned();
        if let Some(stats) = storage_stats.get(&store_name) {
            stats.to_owned()
        } else {
            let store_stats = Arc::new(Mutex::new(InternalStoreStats::default()));
            storage_stats.insert(store_name, store_stats.clone());
            store_stats
        }
    }

    #[allow(unused)]
    pub fn to_inner(self) -> S {
        self.storage
    }
}

impl<S> PerformanceStorage<S>
where
    S: Storage + Space,
    S::KeyValueStore: Store,
    S::BlockStore: Store,
{
    /// Drains the storage stats and returns a summary
    /// of operations as [PerformanceStats].
    pub async fn as_stats(&mut self) -> Result<PerformanceStats> {
        let storage_stats = self.stats.lock().await;
        let mut agg = InternalStoreStats::default();
        for (_, store_stats) in storage_stats.iter() {
            let mut stats = store_stats.lock().await;
            agg.reads.append(&mut stats.reads);
            agg.writes.append(&mut stats.writes);
            agg.removes.append(&mut stats.removes);
            agg.flushes.append(&mut stats.flushes);
            agg.logical_bytes_stored += stats.logical_bytes_stored;
        }

        let mut perf_stats: PerformanceStats = agg.try_into()?;
        perf_stats.physical_bytes_stored = self.storage.get_space_usage().await?;

        Ok(perf_stats)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S> Storage for PerformanceStorage<S>
where
    S: Storage,
    S::KeyValueStore: Store,
    S::BlockStore: Store,
{
    type BlockStore = PerformanceStore<S::BlockStore>;
    type KeyValueStore = PerformanceStore<S::KeyValueStore>;

    async fn get_block_store(&self, name: &str) -> Result<Self::BlockStore> {
        let stats = self.get_store_stats(name).await;
        let store = self.storage.get_block_store(name).await?;
        let block_store = PerformanceStore { store, stats };
        Ok(block_store)
    }

    async fn get_key_value_store(&self, name: &str) -> Result<Self::KeyValueStore> {
        let stats = self.get_store_stats(name).await;
        let store = self.storage.get_key_value_store(name).await?;
        let key_value_store = PerformanceStore { store, stats };
        Ok(key_value_store)
    }
}

#[derive(Debug, Clone)]
pub struct PerformanceStore<S: Store> {
    stats: Arc<Mutex<InternalStoreStats>>,
    store: S,
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S: Store> Store for PerformanceStore<S> {
    async fn read(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let start = Instant::now();
        let value = self.store.read(key).await?;
        let duration = start.elapsed();

        let mut stats = self.stats.lock().await;
        stats.reads.push(duration);
        Ok(value)
    }

    async fn write(&mut self, key: &[u8], bytes: &[u8]) -> Result<Option<Vec<u8>>> {
        let start = Instant::now();
        let result = self.store.write(key, bytes).await;
        let duration = start.elapsed();

        let mut stats = self.stats.lock().await;
        stats.writes.push(duration);
        stats.logical_bytes_stored += bytes.len() as u64;
        result
    }

    async fn remove(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let start = Instant::now();
        let value = self.store.remove(key).await?;
        let duration = start.elapsed();

        let mut stats = self.stats.lock().await;
        if let Some(bytes) = &value {
            stats.logical_bytes_stored -= bytes.len() as u64;
        }
        stats.removes.push(duration);
        Ok(value)
    }

    async fn flush(&self) -> Result<()> {
        let start = Instant::now();
        let result = self.store.flush().await;
        let duration = start.elapsed();
        let mut stats = self.stats.lock().await;
        stats.flushes.push(duration);
        result
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S> EphemeralStorage for PerformanceStorage<S>
where
    S: Storage,
{
    type EphemeralStoreType = <S as EphemeralStorage>::EphemeralStoreType;

    async fn get_ephemeral_store(&self) -> Result<EphemeralStore<Self::EphemeralStoreType>> {
        self.storage.get_ephemeral_store().await
    }
}
