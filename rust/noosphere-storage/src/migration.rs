use crate::{Storage, Store, SPHERE_DB_STORE_NAMES};
use anyhow::Result;
use async_trait::async_trait;
use noosphere_common::ConditionalSync;
use std::pin::Pin;
use tokio_stream::{Stream, StreamExt};

#[cfg(not(target_arch = "wasm32"))]
use crate::{BackupStorage, OpenStorage};

/// An async stream of key/value pairs from an [IterableStore].
#[cfg(not(target_arch = "wasm32"))]
pub type IterableStoreStream<'a> =
    dyn Stream<Item = Result<(Vec<u8>, Option<Vec<u8>>)>> + Send + 'a;
/// An async stream of key/value pairs from an [IterableStore].
#[cfg(target_arch = "wasm32")]
pub type IterableStoreStream<'a> = dyn Stream<Item = Result<(Vec<u8>, Option<Vec<u8>>)>> + 'a;

/// A store that can iterate over all of its entries.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait IterableStore {
    /// Retrieve all key/value pairs from this store as an async stream.
    fn get_all_entries(&self) -> Pin<Box<IterableStoreStream<'_>>>;
}

/// [ExportStorage] [Storage] can be imported by an [ImportStorage]. A [Storage]
/// is [ExportStorage] if its `KeyValueStore` also implements [IterableStore].
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait ExportStorage
where
    Self: Storage,
    <Self as Storage>::KeyValueStore: IterableStore,
{
    /// Returns all active store names in this [Storage].
    async fn get_all_store_names(&self) -> Result<Vec<String>> {
        let mut names = vec![];
        names.extend(SPHERE_DB_STORE_NAMES.iter().map(|name| String::from(*name)));
        Ok(names)
    }
}

impl<S> ExportStorage for S
where
    S: Storage,
    S::KeyValueStore: IterableStore,
{
}

/// A blanket implementation for [Storage]s to import
/// an [ExportStorage] storage.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait ImportStorage<'a, E>
where
    Self: Storage,
    Self::KeyValueStore: Store,
    E: ExportStorage + ConditionalSync + 'a,
    <E as Storage>::KeyValueStore: IterableStore,
{
    /// Copy all stores' entries from `exportable` into this [Storage].
    async fn import(&'a mut self, exportable: &E) -> Result<()> {
        for store_name in exportable.get_all_store_names().await? {
            let mut store = self.get_key_value_store(&store_name).await?;
            let export_store = exportable.get_key_value_store(&store_name).await?;
            let mut stream = export_store.get_all_entries();
            while let Some((key, value)) = stream.try_next().await? {
                if let Some(value) = value {
                    Store::write(&mut store, key.as_ref(), value.as_ref()).await?;
                }
            }
        }
        Ok(())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<'a, T, E> ImportStorage<'a, E> for T
where
    T: Storage,
    T::KeyValueStore: Store,
    E: ExportStorage + ConditionalSync + 'a,
    <E as Storage>::KeyValueStore: IterableStore,
{
}

/// Opens the [Storage] at `path` as storage type `S`, creates a new [Storage]
/// of type `T`, copies over all data, and moves the new storage to `path` upon
/// success.
#[cfg(not(target_arch = "wasm32"))]
pub async fn migrate_storage<S, T>(path: impl AsRef<std::path::Path>) -> Result<T>
where
    for<'a> T: BackupStorage + ImportStorage<'a, S> + OpenStorage,
    <T as Storage>::KeyValueStore: Store,
    S: BackupStorage + OpenStorage + ConditionalSync,
    <S as Storage>::KeyValueStore: IterableStore,
{
    let storage_path = path.as_ref();
    let temp_dir = tempfile::TempDir::new()?;
    let temp_path = temp_dir.path();
    {
        let mut to_storage = T::open(temp_path).await?;
        let from_storage = S::open(storage_path).await?;
        to_storage.import(&from_storage).await?;
    }
    // Note that we use `T: BackupStorage` to restore, which will
    // call `T::backup` on `S`. While we ensure that `S: BackupStorage` also,
    // and this works for filesystems storages, we may need to rethink backups
    // in the context of multiple storage types.
    T::restore(temp_path, storage_path).await?;
    T::open(storage_path).await
}

#[cfg(test)]
mod test {
    use crate::TempStorage;

    use super::*;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test_configure!(run_in_browser);

    /// wasm32: IndexedDbStorage -> MemoryStorage
    /// native: SledStorage -> MemoryStorage
    /// native+rocks: SledStorage -> RocksDbStorage
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    pub async fn it_can_import_export_storages() -> Result<()> {
        noosphere_core_dev::tracing::initialize_tracing(None);

        #[cfg(target_arch = "wasm32")]
        type FromStorage = crate::IndexedDbStorage;
        #[cfg(not(target_arch = "wasm32"))]
        type FromStorage = crate::SledStorage;

        #[cfg(target_arch = "wasm32")]
        type ToStorage = crate::MemoryStorage;
        #[cfg(all(feature = "rocksdb", not(target_arch = "wasm32")))]
        type ToStorage = crate::RocksDbStorage;
        #[cfg(all(not(feature = "rocksdb"), not(target_arch = "wasm32")))]
        type ToStorage = crate::MemoryStorage;

        let from_storage = TempStorage::<FromStorage>::new().await?;
        let mut to_storage = TempStorage::<ToStorage>::new().await?;
        {
            let mut store = from_storage.get_key_value_store("links").await?;
            for n in 0..500u32 {
                let slug = format!("slug-{}", n);
                let bytes = vec![n as u8; 10];
                store.write(slug.as_ref(), bytes.as_ref()).await?;
            }
        }

        to_storage.import(from_storage.as_ref()).await?;

        {
            let store = to_storage.get_key_value_store("links").await?;
            for n in 0..500u32 {
                let slug = format!("slug-{}", n);
                let expected_bytes = vec![n as u8; 10];

                if let Some(bytes) = store.read(slug.as_ref()).await? {
                    assert_eq!(bytes, expected_bytes);
                } else {
                    panic!("Expected key `{n}` to exist in new db");
                }
            }
        }
        Ok(())
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "rocksdb"))]
    #[tokio::test]
    pub async fn it_can_migrate_native_dbs() -> Result<()> {
        noosphere_core_dev::tracing::initialize_tracing(None);
        let temp_dir = tempfile::TempDir::new()?;
        let storage_path = temp_dir.path().join("db");

        {
            let from_storage = crate::SledStorage::new(&storage_path)?;
            let mut store = from_storage.get_key_value_store("links").await?;
            for n in 0..500u32 {
                let slug = format!("slug-{}", n);
                let bytes = vec![n as u8; 10];
                store.write(slug.as_ref(), bytes.as_ref()).await?;
            }
        }

        {
            let to_storage: crate::RocksDbStorage =
                migrate_storage::<crate::SledStorage, crate::RocksDbStorage>(&storage_path).await?;

            let store = to_storage.get_key_value_store("links").await?;
            for n in 0..500u32 {
                let slug = format!("slug-{}", n);
                let expected_bytes = vec![n as u8; 10];

                if let Some(bytes) = store.read(slug.as_ref()).await? {
                    assert_eq!(bytes, expected_bytes);
                } else {
                    panic!("Expected key `{n}` to exist in new db");
                }
            }
        }

        // Ensure we can open via the expected path
        {
            let storage = crate::RocksDbStorage::open(&storage_path).await?;
            let store = storage.get_key_value_store("links").await?;
            for n in 0..500u32 {
                let slug = format!("slug-{}", n);
                let expected_bytes = vec![n as u8; 10];

                if let Some(bytes) = store.read(slug.as_ref()).await? {
                    assert_eq!(bytes, expected_bytes);
                } else {
                    panic!("Expected key `{n}` to exist in new db");
                }
            }
        }

        assert_eq!(
            crate::RocksDbStorage::list_backups(&storage_path)
                .await?
                .len(),
            1,
            "Backup of old DB created."
        );
        Ok(())
    }
}
