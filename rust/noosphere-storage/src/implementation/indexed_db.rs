use crate::store::Store;
use crate::{db::SPHERE_DB_STORE_NAMES, storage::Storage};
use anyhow::{anyhow, Error, Result};
use async_trait::async_trait;
use js_sys::Uint8Array;
use rexie::{
    KeyRange, ObjectStore, Rexie, RexieBuilder, Store as IdbStore, Transaction, TransactionMode,
};
use std::{fmt::Debug, rc::Rc};
use wasm_bindgen::{JsCast, JsValue};

pub const INDEXEDDB_STORAGE_VERSION: u32 = 1;

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
        Self::configure(INDEXEDDB_STORAGE_VERSION, db_name, SPHERE_DB_STORE_NAMES).await
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

#[derive(Clone)]
pub struct IndexedDbStore {
    db: Rc<Rexie>,
    store_name: String,
}

impl IndexedDbStore {
    fn start_transaction(&self, mode: TransactionMode) -> Result<(IdbStore, Transaction)> {
        let tx = self
            .db
            .transaction(&[&self.store_name], mode)
            .map_err(|error| anyhow!("{:?}", error))?;
        let store = tx
            .store(&self.store_name)
            .map_err(|error| anyhow!("{:?}", error))?;

        Ok((store, tx))
    }

    async fn finish_transaction(tx: Transaction) -> Result<()> {
        tx.done().await.map_err(|error| anyhow!("{:?}", error))?;
        Ok(())
    }

    fn bytes_to_typed_array(bytes: &[u8]) -> Result<JsValue> {
        let array = Uint8Array::new_with_length(bytes.len() as u32);
        array.copy_from(&bytes);
        Ok(JsValue::from(array))
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
        let key = IndexedDbStore::bytes_to_typed_array(key)?;

        let maybe_dag = IndexedDbStore::read(&key, &store).await?;

        IndexedDbStore::finish_transaction(tx).await?;

        Ok(maybe_dag)
    }

    async fn write(&mut self, key: &[u8], bytes: &[u8]) -> Result<Option<Vec<u8>>> {
        let (store, tx) = self.start_transaction(TransactionMode::ReadWrite)?;

        let key = IndexedDbStore::bytes_to_typed_array(key)?;
        let value = IndexedDbStore::bytes_to_typed_array(bytes)?;

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

        let key = IndexedDbStore::bytes_to_typed_array(key)?;

        let old_value = IndexedDbStore::read(&key, &store).await?;

        store
            .delete(&key)
            .await
            .map_err(|error| anyhow!("{:?}", error))?;

        IndexedDbStore::finish_transaction(tx).await?;

        Ok(old_value)
    }
}

#[cfg(feature = "performance")]
struct SpaceUsageError(Error);

#[cfg(feature = "performance")]
impl From<JsValue> for SpaceUsageError {
    fn from(value: JsValue) -> SpaceUsageError {
        if let Ok(js_string) = js_sys::JSON::stringify(&value) {
            SpaceUsageError(anyhow!("{}", js_string.as_string().unwrap()))
        } else {
            SpaceUsageError(anyhow!("Could not parse JsValue error as string."))
        }
    }
}

#[cfg(feature = "performance")]
impl From<SpaceUsageError> for Error {
    fn from(value: SpaceUsageError) -> Self {
        value.0
    }
}

#[cfg(feature = "performance")]
use serde;

#[cfg(feature = "performance")]
#[derive(Debug, serde::Deserialize)]
pub struct StorageEstimate {
    pub quota: u64,
    pub usage: u64,
    #[serde(rename = "usageDetails")]
    pub usage_details: Option<UsageDetails>,
}

#[cfg(feature = "performance")]
#[derive(Debug, serde::Deserialize)]
pub struct UsageDetails {
    #[serde(rename = "indexedDB")]
    pub indexed_db: u64,
}

#[cfg(feature = "performance")]
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
            .map_err(|e| <JsValue as Into<SpaceUsageError>>::into(e))?;
        let estimate_obj = wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map_err(|e| <JsValue as Into<SpaceUsageError>>::into(e))?;
        let estimate: StorageEstimate = estimate_obj.into_serde()?;

        if let Some(details) = estimate.usage_details {
            Ok(details.indexed_db)
        } else {
            Ok(estimate.usage)
        }
    }
}
