use crate::{KeyValueStore, Store};
use anyhow::{anyhow, Result};
use async_stream::try_stream;
use async_trait::async_trait;
use noosphere_common::ConditionalSync;
use std::sync::Arc;
use tokio::sync::{Mutex, OnceCell};

/// Provides an [EphemeralStore] that does not persist after dropping.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait EphemeralStorage: ConditionalSync {
    type EphemeralStoreType: KeyValueStore + Disposable;

    async fn get_ephemeral_store(&self) -> Result<EphemeralStore<Self::EphemeralStoreType>>;
}

/// A [Store] that can clear its data after dropping as an [EphemeralStore].
///
/// A [Disposable] store is only cleared when used as an [EphemeralStore].
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait Disposable: Store {
    async fn dispose(&mut self) -> Result<()>;
}

/// Wrapper [Store], ensuring underlying data does not persist.
///
/// As [Store] must be [Sync] and [Clone], [EphemeralStore] clears
/// its data after all references have been dropped.
#[derive(Clone)]
pub struct EphemeralStore<S>(OnceCell<Arc<Mutex<S>>>)
where
    S: KeyValueStore + Disposable + 'static;

impl<S> EphemeralStore<S>
where
    S: KeyValueStore + Disposable + 'static,
{
    pub(crate) fn new(store: S) -> Self {
        Self(OnceCell::from(Arc::new(Mutex::new(store))))
    }

    async fn store(&self) -> Result<tokio::sync::MutexGuard<'_, S>> {
        Ok(self
            .0
            .get()
            .ok_or_else(|| anyhow!("Inner store not set."))?
            .lock()
            .await)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S> Store for EphemeralStore<S>
where
    S: KeyValueStore + Disposable + 'static,
{
    async fn read(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let store = self.store().await?;
        store.read(key).await
    }

    async fn write(&mut self, key: &[u8], bytes: &[u8]) -> Result<Option<Vec<u8>>> {
        let mut store = self.store().await?;
        store.write(key, bytes).await
    }

    async fn remove(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let mut store = self.store().await?;
        store.remove(key).await
    }

    async fn flush(&self) -> Result<()> {
        let store = self.store().await?;
        Store::flush(std::ops::Deref::deref(&store)).await
    }
}

impl<S> From<S> for EphemeralStore<S>
where
    S: KeyValueStore + Disposable + 'static,
{
    fn from(value: S) -> Self {
        EphemeralStore::new(value)
    }
}

impl<S> crate::IterableStore for EphemeralStore<S>
where
    S: crate::IterableStore + KeyValueStore + Disposable + 'static,
{
    fn get_all_entries(&self) -> std::pin::Pin<Box<crate::IterableStoreStream<'_>>> {
        use tokio_stream::StreamExt;
        Box::pin(try_stream! {
            let store = self.store().await?;
            let mut stream = store.get_all_entries();
            while let Some((key, value)) = stream.try_next().await? {
                yield (key, value);
            }
        })
    }
}

impl<S> Drop for EphemeralStore<S>
where
    S: KeyValueStore + Disposable + 'static,
{
    fn drop(&mut self) {
        if let Some(store_arc) = self.0.take() {
            if let Some(store_mutex) = Arc::into_inner(store_arc) {
                let mut store = store_mutex.into_inner();
                noosphere_common::spawn_no_wait(async move {
                    if let Err(e) = store.dispose().await {
                        error!("Error disposing EphemeralStore: {}", e);
                    }
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EphemeralStorage, IterableStore, NonPersistentStorage, OpenStorage,
        PreferredPlatformStorage, Storage, EPHEMERAL_STORE,
    };
    use noosphere_core_dev::tracing::initialize_tracing;
    use std::path::Path;
    use tokio_stream::StreamExt;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_clears_ephemeral_storage_on_store_drop() -> Result<()> {
        initialize_tracing(None);

        let storage = NonPersistentStorage::<PreferredPlatformStorage>::new().await?;
        for _ in 0..3 {
            let mut ephemeral_store = storage.get_ephemeral_store().await?;
            for n in 0..10 {
                ephemeral_store
                    .write(format!("{}", n).as_ref(), &vec![2; 100])
                    .await?;
            }
        }

        // Wait for destructors to complete asynchronously.
        noosphere_common::helpers::wait(1).await;

        #[cfg(all(not(target_arch = "wasm32"), not(feature = "rocksdb")))]
        {
            // Sled's storage space still grows on a new DB when removing trees.
            // Otherwise we could also test deletion via [crate::Space].
            let db = storage.inner();
            assert_eq!(db.tree_names().len(), 1, "only default tree persists.");
        }

        {
            // Ensure there's no extra data in the ephemeral space
            // (IndexedDbStorage, RocksDbStorage)
            let store = storage.get_key_value_store(EPHEMERAL_STORE).await?;
            let mut stream = store.get_all_entries();
            assert!(
                stream.try_next().await?.is_none(),
                "ephemeral store should have no entries."
            );
        }
        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn test_ephemeral_store_isolation() -> Result<()> {
        initialize_tracing(None);

        let storage = NonPersistentStorage::<PreferredPlatformStorage>::new().await?;

        let mut ephemeral_stores: Vec<
            EphemeralStore<<PreferredPlatformStorage as EphemeralStorage>::EphemeralStoreType>,
        > = vec![];
        for _ in 0..2 {
            let mut ephemeral_store = storage.get_ephemeral_store().await?;
            for n in 0..10 {
                ephemeral_store
                    .write(format!("{}", n).as_ref(), &vec![1; 100])
                    .await?;
            }

            {
                let mut stream = ephemeral_store.get_all_entries();
                let mut entries = vec![];
                while let Some(entry) = stream.try_next().await? {
                    entries.push(entry);
                }
                assert_eq!(
                    entries.len(),
                    10,
                    "get_all_entries() should be scoped to this store."
                );
            }
            ephemeral_stores.push(ephemeral_store);
        }

        ephemeral_stores.pop();
        let store = ephemeral_stores.pop().unwrap();
        assert_eq!(
            store.read(format!("0").as_ref()).await?.unwrap(),
            vec![1; 100],
            "ephemeral stores can be dropped without affecting other ephemeral stores."
        );
        Ok(())
    }

    /// Circumvent using [EphemeralStore] to ensure ephemeral data is wiped
    /// on storage drop/init in the event [EphemeralStore] cleanups do not occur.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_clears_ephemeral_storage_on_startup() -> Result<()> {
        #[cfg(target_arch = "wasm32")]
        let db_path: String = witty_phrase_generator::WPGen::new()
            .with_words(3)
            .unwrap()
            .into_iter()
            .map(|word| String::from(word))
            .collect();

        #[cfg(not(target_arch = "wasm32"))]
        let _temp_dir = tempfile::TempDir::new()?;
        #[cfg(not(target_arch = "wasm32"))]
        let db_path = _temp_dir.path();

        #[cfg(any(target_arch = "wasm32", feature = "rocksdb"))]
        let result = test_partition_based_ephemeral_storage_startup(db_path).await?;
        #[cfg(all(not(target_arch = "wasm32"), not(feature = "rocksdb")))]
        let result = test_tree_based_ephemeral_storage_startup(db_path).await?;

        Ok(result)
    }

    /// IndexedDbStorage and RocksDbStorage write to the shared ephemeral space.
    /// Write directly to the ephemeral store to ensure it is cleaned up
    /// on storage drop/init.
    #[cfg(any(target_arch = "wasm32", feature = "rocksdb"))]
    async fn test_partition_based_ephemeral_storage_startup<P: AsRef<Path>>(
        db_path: P,
    ) -> Result<()> {
        {
            let storage = PreferredPlatformStorage::open(db_path.as_ref()).await?;
            let mut store = storage.get_key_value_store(EPHEMERAL_STORE).await?;
            store.write(&[0], &[11]).await?;
            assert_eq!(store.read(&vec![0]).await?, Some(vec![11]));
        }

        let storage = PreferredPlatformStorage::open(db_path.as_ref()).await?;
        let store = storage.get_key_value_store(EPHEMERAL_STORE).await?;
        let mut stream = store.get_all_entries();
        assert!(
            stream.try_next().await?.is_none(),
            "ephemeral store should have no entries."
        );
        Ok(())
    }

    // SledStorage uses ephemeral trees. Ensure the trees do not exist after
    // reinitialization.
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "rocksdb")))]
    async fn test_tree_based_ephemeral_storage_startup<P: AsRef<Path>>(db_path: P) -> Result<()> {
        let mut store_name = String::from(crate::implementation::EPHEMERAL_SLED_PREFIX);
        store_name.push_str("-1234567890");
        {
            let storage = PreferredPlatformStorage::open(db_path.as_ref()).await?;
            let mut store = storage.get_key_value_store(&store_name).await?;
            store.write(&[1], &[11]).await?;
            assert_eq!(store.read(&vec![1]).await?, Some(vec![11]));
        }

        let storage = PreferredPlatformStorage::open(db_path.as_ref()).await?;
        let store = storage.get_key_value_store(&store_name).await?;
        let mut stream = store.get_all_entries();
        assert!(
            stream.try_next().await?.is_none(),
            "ephemeral store should have no entries."
        );
        Ok(())
    }
}
