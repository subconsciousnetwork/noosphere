use crate::{Disposable, Store};
use anyhow::Result;
use async_stream::try_stream;
use async_trait::async_trait;

/// Maps all [Store] method calls with a key to a prefixed form.
#[derive(Clone)]
pub struct PartitionedStore<S>
where
    S: Store,
{
    store: S,
    partition_key: Vec<u8>,
    end_partition_key: Vec<u8>,
}

impl<S> PartitionedStore<S>
where
    S: Store,
{
    pub fn new(store: S) -> Self {
        let prefix: Vec<u8> = format!("{:0<10}-", rand::random::<u32>()).into();
        Self::with_partition_key(store, prefix)
    }

    pub fn with_partition_key(store: S, partition_key: Vec<u8>) -> Self {
        let mut end_partition_key = partition_key.clone();
        end_partition_key.push(u8::MAX);
        Self {
            store,
            partition_key,
            end_partition_key,
        }
    }

    /// Returns `bool` indicating whether `key` is within
    /// the partition key space.
    fn partition_key_contains(&self, key: &[u8]) -> bool {
        key.starts_with(self.partition_key.as_slice())
    }

    pub fn get_key_range(&self) -> (&Vec<u8>, &Vec<u8>) {
        (&self.partition_key, &self.end_partition_key)
    }

    pub fn inner(&self) -> &S {
        &self.store
    }

    fn map_key(&self, key: &[u8]) -> Vec<u8> {
        let mut new_key = self.partition_key.clone();
        new_key.extend(key);
        new_key
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S> Store for PartitionedStore<S>
where
    S: Store,
{
    async fn read(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        self.store.read(&self.map_key(key)).await
    }

    async fn write(&mut self, key: &[u8], bytes: &[u8]) -> Result<Option<Vec<u8>>> {
        self.store.write(&self.map_key(key), bytes).await
    }

    async fn remove(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        self.store.remove(&self.map_key(key)).await
    }

    async fn flush(&self) -> Result<()> {
        self.store.flush().await
    }
}

impl<S> crate::IterableStore for PartitionedStore<S>
where
    S: Store + crate::IterableStore,
{
    fn get_all_entries(&self) -> std::pin::Pin<Box<crate::IterableStoreStream<'_>>> {
        use tokio_stream::StreamExt;
        Box::pin(try_stream! {
            let mut stream = self.store.get_all_entries();
            while let Some((key, value)) = stream.try_next().await? {
                if self.partition_key_contains(&key) {
                    yield (key, value);
                }
            }
        })
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S> Disposable for PartitionedStore<S>
where
    S: Store + Disposable,
{
    async fn dispose(&mut self) -> Result<()> {
        self.store.dispose().await
    }
}
