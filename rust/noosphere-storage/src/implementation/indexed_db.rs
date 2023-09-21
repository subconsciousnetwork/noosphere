use crate::store::Store;
use crate::{
    db::{EPHEMERAL_STORE, SPHERE_DB_STORE_NAMES},
    storage::Storage,
    PartitionedStore,
};
use anyhow::{anyhow, Result};
use async_stream::try_stream;
use async_trait::async_trait;
use js_sys::Uint8Array;
use noosphere_common::ConditionalSend;
use rexie::{
    KeyRange, ObjectStore, Rexie, RexieBuilder, Store as IdbStore, Transaction, TransactionMode,
};
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, path::Path, rc::Rc};
use wasm_bindgen::{JsCast, JsValue};

use js_utils::*;

pub const INDEXEDDB_STORAGE_VERSION: u32 = 2;

#[derive(Clone)]
pub struct IndexedDbStorage {
    db: Rc<Rexie>,
    name: String,
}

impl Debug for IndexedDbStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndexedDbStorage").finish()
    }
}

impl IndexedDbStorage {
    pub async fn new(db_name: &str) -> Result<Self> {
        let storage =
            Self::configure(INDEXEDDB_STORAGE_VERSION, db_name, SPHERE_DB_STORE_NAMES).await?;
        storage.clear_ephemeral().await?;
        Ok(storage)
    }

    async fn configure(version: u32, db_name: &str, store_names: &[&str]) -> Result<Self> {
        let mut builder = RexieBuilder::new(db_name).version(version);

        for name in store_names {
            builder = builder.add_object_store(ObjectStore::new(name).auto_increment(false));
        }

        let db = builder
            .build()
            .await
            .map_err(|error| anyhow!("{:?}", error))?;

        Ok(IndexedDbStorage {
            db: Rc::new(db),
            name: db_name.to_owned(),
        })
    }

    async fn get_store(&self, name: &str) -> Result<IndexedDbStore> {
        if self
            .db
            .store_names()
            .iter()
            .find(|val| val.as_str() == name)
            .is_none()
        {
            return Err(anyhow!("No such store named {}", name));
        }

        Ok(IndexedDbStore {
            db: self.db.clone(),
            store_name: name.to_string(),
        })
    }

    /// Closes and clears the database from origin storage.
    pub async fn clear(self) -> Result<()> {
        let name = self.name;
        let db = Rc::into_inner(self.db)
            .ok_or_else(|| anyhow!("Could not unwrap inner during database clear."))?;
        db.close();
        Rexie::delete(&name)
            .await
            .map_err(|error| anyhow!("{:?}", error))
    }

    /// Wipes the "ephemeral" column family.
    async fn clear_ephemeral(&self) -> Result<()> {
        let ephemeral_store = self.get_store(EPHEMERAL_STORE).await?;
        ephemeral_store.clear().await
    }
}

#[async_trait(?Send)]
impl Storage for IndexedDbStorage {
    type BlockStore = IndexedDbStore;

    type KeyValueStore = IndexedDbStore;

    async fn get_block_store(&self, name: &str) -> Result<Self::BlockStore> {
        self.get_store(name).await
    }

    async fn get_key_value_store(&self, name: &str) -> Result<Self::KeyValueStore> {
        self.get_store(name).await
    }
}

#[async_trait(?Send)]
impl crate::ops::OpenStorage for IndexedDbStorage {
    async fn open<P: AsRef<Path> + ConditionalSend>(path: P) -> Result<Self> {
        IndexedDbStorage::new(
            path.as_ref()
                .to_str()
                .ok_or_else(|| anyhow!("Could not stringify path."))?,
        )
        .await
    }
}

#[async_trait(?Send)]
impl crate::EphemeralStorage for IndexedDbStorage {
    type EphemeralStoreType = EphemeralIndexedDbStore;

    async fn get_ephemeral_store(&self) -> Result<crate::EphemeralStore<Self::EphemeralStoreType>> {
        let store = PartitionedStore::new(IndexedDbStore {
            db: self.db.clone(),
            store_name: EPHEMERAL_STORE.to_owned(),
        });
        Ok(crate::EphemeralIndexedDbStore::new(store).into())
    }
}

#[derive(Clone)]
pub struct IndexedDbStore {
    db: Rc<Rexie>,
    store_name: String,
}

impl IndexedDbStore {
    pub(crate) fn start_transaction(
        &self,
        mode: TransactionMode,
    ) -> Result<(IdbStore, Transaction)> {
        let tx = self
            .db
            .transaction(&[&self.store_name], mode)
            .map_err(|error| anyhow!("{:?}", error))?;
        let store = tx
            .store(&self.store_name)
            .map_err(|error| anyhow!("{:?}", error))?;

        Ok((store, tx))
    }

    pub(crate) async fn finish_transaction(tx: Transaction) -> Result<()> {
        tx.done().await.map_err(|error| anyhow!("{:?}", error))?;
        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        let (store, tx) = self.start_transaction(TransactionMode::ReadWrite)?;
        store
            .clear()
            .await
            .map_err(|error| anyhow!("{:?}", error))?;
        IndexedDbStore::finish_transaction(tx).await?;
        Ok(())
    }

    async fn remove_range(&self, from: &[u8], to: &[u8]) -> Result<()> {
        let (store, tx) = self.start_transaction(TransactionMode::ReadWrite)?;

        let lower = bytes_to_typed_array(from)?;
        let upper = bytes_to_typed_array(to)?;
        let key_range =
            KeyRange::bound(&lower, &upper, false, false).map_err(|e| anyhow!("{:?}", e))?;

        store
            .delete(key_range.as_ref())
            .await
            .map_err(|error| anyhow!("{:?}", error))?;

        IndexedDbStore::finish_transaction(tx).await?;
        Ok(())
    }

    async fn contains(key: &JsValue, store: &IdbStore) -> Result<bool> {
        let count = store
            .count(Some(
                &KeyRange::only(key).map_err(|error| anyhow!("{:?}", error))?,
            ))
            .await
            .map_err(|error| anyhow!("{:?}", error))?;
        Ok(count > 0)
    }

    async fn read(key: &JsValue, store: &IdbStore) -> Result<Option<Vec<u8>>> {
        Ok(match IndexedDbStore::contains(&key, &store).await? {
            true => Some(
                store
                    .get(&key)
                    .await
                    .map_err(|error| anyhow!("{:?}", error))?
                    .dyn_into::<Uint8Array>()
                    .map_err(|error| anyhow!("{:?}", error))?
                    .to_vec(),
            ),
            false => None,
        })
    }
}

#[async_trait(?Send)]
impl Store for IndexedDbStore {
    async fn read(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let (store, tx) = self.start_transaction(TransactionMode::ReadOnly)?;
        let key = bytes_to_typed_array(key)?;

        let maybe_dag = IndexedDbStore::read(&key, &store).await?;

        IndexedDbStore::finish_transaction(tx).await?;

        Ok(maybe_dag)
    }

    async fn write(&mut self, key: &[u8], bytes: &[u8]) -> Result<Option<Vec<u8>>> {
        let (store, tx) = self.start_transaction(TransactionMode::ReadWrite)?;

        let key = bytes_to_typed_array(key)?;
        let value = bytes_to_typed_array(bytes)?;

        let old_bytes = IndexedDbStore::read(&key, &store).await?;

        store
            .put(&value, Some(&key))
            .await
            .map_err(|error| anyhow!("{:?}", error))?;

        IndexedDbStore::finish_transaction(tx).await?;

        Ok(old_bytes)
    }

    async fn remove(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let (store, tx) = self.start_transaction(TransactionMode::ReadWrite)?;

        let key = bytes_to_typed_array(key)?;

        let old_value = IndexedDbStore::read(&key, &store).await?;

        store
            .delete(&key)
            .await
            .map_err(|error| anyhow!("{:?}", error))?;

        IndexedDbStore::finish_transaction(tx).await?;

        Ok(old_value)
    }
}

impl crate::IterableStore for IndexedDbStore {
    fn get_all_entries(&self) -> std::pin::Pin<Box<crate::IterableStoreStream<'_>>> {
        Box::pin(try_stream! {
            let (store, tx) = self.start_transaction(TransactionMode::ReadWrite)?;
            let limit = 100;
            let mut offset = 0;
            loop {
                let results = store.get_all(None, Some(limit), Some(offset), None).await
                    .map_err(|error| anyhow!("{:?}", error))?;
                let count = results.len();
                if count == 0 {
                    IndexedDbStore::finish_transaction(tx).await?;
                    break;
                }

                offset += count as u32;

                for (key_js, value_js) in results {
                    yield (
                        typed_array_to_bytes(JsValue::from(Uint8Array::new(&key_js)))?,
                        typed_array_to_bytes(value_js)?
                    );
                }
            }
        })
    }
}

/// A [IndexedDbStore] that does not persist data after dropping.
/// Can be created from [IndexedDbStorage::get_ephemeral_store].
#[derive(Clone)]
pub struct EphemeralIndexedDbStore {
    store: PartitionedStore<IndexedDbStore>,
}

impl EphemeralIndexedDbStore {
    pub(crate) fn new(store: PartitionedStore<IndexedDbStore>) -> Self {
        EphemeralIndexedDbStore { store }
    }
}

#[async_trait(?Send)]
impl Store for EphemeralIndexedDbStore {
    async fn read(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        self.store.read(key).await
    }

    async fn write(&mut self, key: &[u8], bytes: &[u8]) -> Result<Option<Vec<u8>>> {
        self.store.write(key, bytes).await
    }

    async fn remove(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        self.store.remove(key).await
    }

    async fn flush(&self) -> Result<()> {
        self.store.flush().await
    }
}

#[async_trait(?Send)]
impl crate::Disposable for EphemeralIndexedDbStore {
    async fn dispose(&mut self) -> Result<()> {
        let (start_key, end_key) = self.store.get_key_range();
        self.store.inner().remove_range(start_key, end_key).await
    }
}

#[async_trait(?Send)]
impl crate::IterableStore for EphemeralIndexedDbStore {
    fn get_all_entries(&self) -> std::pin::Pin<Box<crate::IterableStoreStream<'_>>> {
        self.store.get_all_entries()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageEstimate {
    pub quota: u64,
    pub usage: u64,
    #[serde(rename = "usageDetails")]
    pub usage_details: Option<UsageDetails>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UsageDetails {
    #[serde(rename = "indexedDB")]
    pub indexed_db: u64,
}

#[async_trait(?Send)]
impl crate::Space for IndexedDbStorage {
    /// Returns an estimate of disk usage of the IndexedDb instance.
    /// Note this includes all storage usage for the active origin -- it is up
    /// to the consumer to clear unwanted databases for more accurate reporting.
    /// https://developer.mozilla.org/en-US/docs/Web/API/StorageManager/estimate
    ///
    /// A benign(?) warning is emitted by an underlying dependency from this:
    /// https://github.com/DioxusLabs/cli/issues/62
    async fn get_space_usage(&self) -> Result<u64> {
        let window = web_sys::window().ok_or_else(|| anyhow!("No window found."))?;
        let storage = window.navigator().storage();
        let promise = storage
            .estimate()
            .map_err(|e| <JsValue as Into<JsError>>::into(e))?;
        let estimate_obj = wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map_err(|e| <JsValue as Into<JsError>>::into(e))?;
        let estimate: StorageEstimate = serde_wasm_bindgen::from_value(estimate_obj)
            .map_err(|e| <serde_wasm_bindgen::Error as Into<JsError>>::into(e))?;

        if let Some(details) = estimate.usage_details {
            Ok(details.indexed_db)
        } else {
            Ok(estimate.usage)
        }
    }
}

mod js_utils {
    use anyhow::{anyhow, Error, Result};
    use js_sys::Uint8Array;
    use wasm_bindgen::{JsCast, JsValue};

    pub struct JsError(Error);

    impl From<JsValue> for JsError {
        fn from(value: JsValue) -> JsError {
            if let Ok(js_string) = js_sys::JSON::stringify(&value) {
                JsError(anyhow!("{}", js_string.as_string().unwrap()))
            } else {
                JsError(anyhow!("Could not parse JsValue error as string."))
            }
        }
    }

    impl From<serde_wasm_bindgen::Error> for JsError {
        fn from(value: serde_wasm_bindgen::Error) -> JsError {
            let js_value: JsValue = value.into();
            js_value.into()
        }
    }

    impl From<JsError> for Error {
        fn from(value: JsError) -> Self {
            value.0
        }
    }

    pub fn bytes_to_typed_array(bytes: &[u8]) -> Result<JsValue> {
        let array = Uint8Array::new_with_length(bytes.len() as u32);
        array.copy_from(&bytes);
        Ok(JsValue::from(array))
    }

    pub fn typed_array_to_bytes(js_value: JsValue) -> Result<Vec<u8>> {
        Ok(js_value
            .dyn_into::<Uint8Array>()
            .map_err(|error| anyhow!("{:?}", error))?
            .to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{key_value::KeyValueStore, LINK_STORE};
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
    wasm_bindgen_test_configure!(run_in_browser);

    #[derive(Clone)]
    pub struct IndexedDbStorageV1 {
        db: Rc<Rexie>,
    }

    impl IndexedDbStorageV1 {
        pub async fn new(db_name: &str) -> Result<Self> {
            const V1_STORES: [&str; 4] = ["blocks", "links", "versions", "metadata"];
            let mut builder = RexieBuilder::new(db_name).version(1);

            for name in V1_STORES {
                builder = builder.add_object_store(ObjectStore::new(name).auto_increment(false));
            }

            let db = builder
                .build()
                .await
                .map_err(|error| anyhow!("{:?}", error))?;

            Ok(IndexedDbStorageV1 { db: Rc::new(db) })
        }

        async fn get_store(&self, name: &str) -> Result<IndexedDbStore> {
            if self
                .db
                .store_names()
                .iter()
                .find(|val| val.as_str() == name)
                .is_none()
            {
                return Err(anyhow!("No such store named {}", name));
            }

            Ok(IndexedDbStore {
                db: self.db.clone(),
                store_name: name.to_string(),
            })
        }
    }

    /// Triggering a migration from version upgrade can take several
    /// seconds (8s on average, can be as low as 2 seconds, or as high as 30s).
    #[wasm_bindgen_test]
    #[ignore]
    async fn it_can_upgrade_from_v1() -> Result<()> {
        // noosphere_core_dev::tracing::initialize_tracing(None);
        let db_name = format!("{}_v1_test", rand::random::<u32>());
        let key = String::from("foo");
        let value = String::from("bar");
        {
            let storage_v1 = IndexedDbStorageV1::new(&db_name).await?;
            let mut store_v1 = storage_v1.get_store(LINK_STORE).await?;
            store_v1.set_key(&key, &value).await?;
        }

        let start = instant::Instant::now();
        let storage_v2 = IndexedDbStorage::new(&db_name).await?;
        info!("Store migrated (t={}ms)", start.elapsed().as_millis());
        let store_v2 = storage_v2.get_store(LINK_STORE).await?;
        assert_eq!(store_v2.get_key::<_, String>(&key).await?.unwrap(), value);
        Ok(())
    }
}
