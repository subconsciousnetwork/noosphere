use anyhow::{anyhow, Result};
use async_stream::try_stream;
use async_trait::async_trait;
use noosphere_common::{ConditionalSend, ConditionalSync};
use serde::{de::DeserializeOwned, Serialize};
use tokio_stream::Stream;

use crate::{
    EphemeralStorage, EphemeralStore, ExtendStore, IterableStore, KeyValueStore, MemoryStore, Store,
};

/// Configurations for [StoreBuffer] on handling the underyling
/// scratch storage space.
pub enum StoreBufferStrategy<'a, S>
where
    S: EphemeralStorage + 'a,
{
    /// Use [MemoryStore] for the duration of the [StoreBuffer].
    Memory,
    /// Use [MemoryStore] unless the [StoreBuffer] exceeds `limit`
    /// items. Once the limit has been exceeded, the inner store
    /// transitions to an [EphemeralStore] from the [EphemeralStorage] `provider`,
    /// capable of providing disk-storage for larger workloads.
    ProviderAtItemLimit { provider: &'a S, limit: usize },
}

/// An ordered collection of `T` backed by a [Store].
///
/// [StoreBuffer] persists to an underlying [Store], configurable
/// by [StoreBufferStrategy].
///
/// If a store writes to disk, this can be used to process large amounts of
/// ordered data without storing it entirely in memory.
pub struct StoreBuffer<'a, S, T>
where
    S: EphemeralStorage + 'a,
    <S as EphemeralStorage>::EphemeralStoreType: 'static,
{
    next: usize,
    store: AdaptiveStore<MemoryStore, EphemeralStore<S::EphemeralStoreType>>,
    strategy: StoreBufferStrategy<'a, S>,
    _marker: std::marker::PhantomData<T>,
}

impl<'a, S, T> StoreBuffer<'a, S, T>
where
    S: EphemeralStorage + 'a,
    <S as EphemeralStorage>::EphemeralStoreType: 'static,
    T: DeserializeOwned + Serialize + ConditionalSend,
{
    pub fn new(strategy: StoreBufferStrategy<'a, S>) -> Self {
        Self {
            next: 0,
            strategy,
            store: AdaptiveStore::new(MemoryStore::default()),
            _marker: std::marker::PhantomData,
        }
    }

    /// Returns the number of items in the collection.
    pub fn len(&self) -> usize {
        self.next
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the item at `index` or `None` if out of range.
    pub async fn get(&self, index: usize) -> Result<Option<T>> {
        if index < self.next {
            let key = self.key(index);
            self.store.get_key(&key).await
        } else {
            Ok(None)
        }
    }

    /// Appends an element to the end of the collection.
    pub async fn push(&mut self, item: T) -> Result<()> {
        if let StoreBufferStrategy::ProviderAtItemLimit { provider, limit } = self.strategy {
            if !self.store.is_upgraded() && self.len() == limit {
                let store = provider.get_ephemeral_store().await?;
                self.store.upgrade(store).await?;
            }
        }
        let key = self.key(self.next);
        self.store.set_key(&key, item).await?;
        self.next += 1;
        Ok(())
    }

    /// Removes the last element in the collection and returns it, or `None` if empty.
    pub async fn pop(&mut self) -> Result<Option<T>> {
        if self.is_empty() {
            return Ok(None);
        }
        assert!(self.next > 0);
        let key = self.key(self.next - 1);
        let result = self.store.unset_key(&key).await?;
        self.next -= 1;
        Ok(result)
    }

    /// Streams this buffer's items.
    pub fn as_forward_stream(
        &'a self,
    ) -> std::pin::Pin<Box<dyn Stream<Item = Result<T, anyhow::Error>> + 'a>> {
        let len = self.len();
        Box::pin(try_stream! {
            for index in 0..len {
                if let Some(value) = self.get(index).await? {
                    yield value;
                }
            }
        })
    }

    /// Streams this buffer's items in reverse.
    pub fn as_reverse_stream(
        &'a self,
    ) -> std::pin::Pin<Box<dyn Stream<Item = Result<T, anyhow::Error>> + 'a>> {
        let len = self.len();
        Box::pin(try_stream! {
            for n in 0..len {
                let index = len - n - 1;
                if let Some(value) = self.get(index).await? {
                    yield value;
                }
            }
        })
    }

    fn key(&self, key: usize) -> String {
        format!("{}", key)
    }
}

#[derive(Clone)]
enum AdaptiveStoreEither<S1, S2> {
    Initial(S1),
    Upgraded(S2),
}

/// [Store] that wraps an underlying store `S1`, and can upgrade to store `S2`.
///
/// Upgrading is a one-way process, where a new store `S2` copies data from
/// the initial store `S1`, becoming the new underlying store.
#[derive(Clone)]
pub struct AdaptiveStore<S1, S2>
where
    S1: Store + IterableStore + ConditionalSync,
    S2: ExtendStore,
{
    store: AdaptiveStoreEither<S1, S2>,
}

impl<S1, S2> AdaptiveStore<S1, S2>
where
    S1: Store + IterableStore + ConditionalSync,
    S2: ExtendStore,
{
    pub fn new(initial_store: S1) -> Self {
        Self {
            store: AdaptiveStoreEither::Initial(initial_store),
        }
    }

    /// Whether or not this [AdaptiveStore] has been upgraded.
    pub fn is_upgraded(&self) -> bool {
        matches!(self.store, AdaptiveStoreEither::Upgraded(_))
    }

    /// Swaps the initial store with the upgraded store.
    /// Throws an error if this is called more than once.
    pub async fn upgrade(&mut self, upgraded_store: S2) -> Result<()> {
        let mut upgraded_store = upgraded_store;
        match &self.store {
            AdaptiveStoreEither::Initial(initial_store) => {
                upgraded_store.extend(initial_store).await?;
                self.store = AdaptiveStoreEither::Upgraded(upgraded_store);
                Ok(())
            }
            AdaptiveStoreEither::Upgraded(_) => Err(anyhow!("Unexpected AdaptiveStore state.")),
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S1, S2> Store for AdaptiveStore<S1, S2>
where
    S1: Store + IterableStore + ConditionalSync,
    S2: ExtendStore,
{
    async fn read(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        match &self.store {
            AdaptiveStoreEither::Initial(store) => store.read(key).await,
            AdaptiveStoreEither::Upgraded(store) => store.read(key).await,
        }
    }

    async fn write(&mut self, key: &[u8], bytes: &[u8]) -> Result<Option<Vec<u8>>> {
        match &mut self.store {
            AdaptiveStoreEither::Initial(ref mut store) => store.write(key, bytes).await,
            AdaptiveStoreEither::Upgraded(ref mut store) => store.write(key, bytes).await,
        }
    }

    async fn remove(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        match &mut self.store {
            AdaptiveStoreEither::Initial(ref mut store) => store.remove(key).await,
            AdaptiveStoreEither::Upgraded(ref mut store) => store.remove(key).await,
        }
    }

    async fn flush(&self) -> Result<()> {
        match &self.store {
            AdaptiveStoreEither::Initial(store) => store.flush().await,
            AdaptiveStoreEither::Upgraded(store) => store.flush().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MemoryStore, NonPersistentStorage, PreferredPlatformStorage};
    use tokio_stream::StreamExt;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    pub async fn test_adaptive_store() -> Result<()> {
        let memory_store = MemoryStore::default();
        let storage = NonPersistentStorage::<PreferredPlatformStorage>::new().await?;
        let ephemeral_store = storage.get_ephemeral_store().await?;

        let mut adaptive_store = AdaptiveStore::new(memory_store);

        for n in 0..3 {
            adaptive_store
                .write(format!("{}", n).as_ref(), &vec![n; 50])
                .await?;
        }

        adaptive_store.upgrade(ephemeral_store).await?;

        for n in 3..6 {
            adaptive_store
                .write(format!("{}", n).as_ref(), &vec![n; 50])
                .await?;
        }

        for n in 0..6 {
            assert_eq!(
                adaptive_store
                    .read(format!("{}", n).as_ref())
                    .await?
                    .unwrap(),
                vec![n as u8; 50]
            );
        }

        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    pub async fn test_store_buffer_basics() -> Result<()> {
        let limit = 5;
        let storage = NonPersistentStorage::<PreferredPlatformStorage>::new().await?;
        let mut buffer = StoreBuffer::new(StoreBufferStrategy::ProviderAtItemLimit {
            provider: &storage,
            limit,
        });

        for n in 0..10 {
            buffer.push(n * 10).await?;
        }
        assert_eq!(buffer.len(), 10);
        assert_eq!(buffer.get(100).await?, None);
        for n in 0..10 {
            assert_eq!(buffer.get(n).await?, Some(n * 10));
        }
        assert_eq!(buffer.pop().await?, Some(90));
        assert_eq!(buffer.pop().await?, Some(80));
        assert_eq!(buffer.len(), 8);
        assert_eq!(buffer.get(9).await?, None);

        let mut stream = buffer.as_forward_stream();
        let mut i = 0;
        while let Some(item) = stream.try_next().await? {
            assert_eq!(item, i * 10);
            i += 1;
        }
        assert_eq!(i, 8);
        assert_eq!(buffer.len(), 8);

        let mut stream = buffer.as_reverse_stream();
        let mut i = 8;
        while let Some(item) = stream.try_next().await? {
            i -= 1;
            assert_eq!(item, i * 10);
        }
        assert_eq!(i, 0);

        Ok(())
    }
}
