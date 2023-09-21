use crate::store::Store;
use crate::{db::SPHERE_DB_STORE_NAMES, storage::Storage};
use anyhow::{anyhow, Error, Result};
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

/// Current SphereDb migration version.
pub const INDEXEDDB_STORAGE_VERSION: u32 = 1;

/// An [IndexedDB](https://web.dev/indexeddb/)-backed implementation for `wasm32-unknown-unknown` targets.
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
    /// Open or create a database with key `db_name`.
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
        Self::delete(&name).await
    }

    /// Deletes database with key `db_name` from origin storage.
    pub async fn delete(db_name: &str) -> Result<()> {
        Rexie::delete(db_name)
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
impl crate::ops::DeleteStorage for IndexedDbStorage {
    async fn delete<P: AsRef<Path> + ConditionalSend>(path: P) -> Result<()> {
        Self::delete(
            path.as_ref()
                .to_str()
                .ok_or_else(|| anyhow!("Could not stringify path."))?,
        )
        .await
    }
}

#[derive(Clone)]
/// A [Store] implementation for [IndexedDbStorage].
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

    async fn contains(key: &[u8], store: &IdbStore) -> Result<bool> {
        let key_js = bytes_to_typed_array(key)?;
        let count = store
            .count(Some(
                &KeyRange::only(&key_js).map_err(|error| anyhow!("{:?}", error))?,
            ))
            .await
            .map_err(|error| anyhow!("{:?}", error))?;
        Ok(count > 0)
    }

    async fn read(key: &[u8], store: &IdbStore) -> Result<Option<Vec<u8>>> {
        let key_js = bytes_to_typed_array(key)?;
        Ok(match IndexedDbStore::contains(&key, &store).await? {
            true => Some(typed_array_to_bytes(
                store
                    .get(&key_js)
                    .await
                    .map_err(|error| anyhow!("{:?}", error))?,
            )?),
            false => None,
        })
    }

    async fn put(key: &[u8], value: &[u8], store: &IdbStore) -> Result<()> {
        let key_js = bytes_to_typed_array(key)?;
        let value_js = bytes_to_typed_array(value)?;
        store
            .put(&value_js, Some(&key_js))
            .await
            .map_err(|error| anyhow!("{:?}", error))?;
        Ok(())
    }

    async fn delete(key: &[u8], store: &IdbStore) -> Result<()> {
        let key_js = bytes_to_typed_array(key)?;
        store
            .delete(&key_js)
            .await
            .map_err(|error| anyhow!("{:?}", error))?;
        Ok(())
    }
}

#[async_trait(?Send)]
impl Store for IndexedDbStore {
    async fn read(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let (store, tx) = self.start_transaction(TransactionMode::ReadOnly)?;
        let maybe_dag = IndexedDbStore::read(key, &store).await?;
        IndexedDbStore::finish_transaction(tx).await?;
        Ok(maybe_dag)
    }

    async fn write(&mut self, key: &[u8], bytes: &[u8]) -> Result<Option<Vec<u8>>> {
        let (store, tx) = self.start_transaction(TransactionMode::ReadWrite)?;
        let old_bytes = IndexedDbStore::read(&key, &store).await?;
        IndexedDbStore::put(key, bytes, &store).await?;
        IndexedDbStore::finish_transaction(tx).await?;
        Ok(old_bytes)
    }

    async fn remove(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let (store, tx) = self.start_transaction(TransactionMode::ReadWrite)?;
        let old_value = IndexedDbStore::read(key, &store).await?;
        IndexedDbStore::delete(key, &store).await?;
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
                        Some(typed_array_to_bytes(value_js)?)
                    );
                }
            }
        })
    }
}

struct JsError(Error);

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

#[derive(Debug, Serialize, Deserialize)]
struct StorageEstimate {
    pub quota: u64,
    pub usage: u64,
    #[serde(rename = "usageDetails")]
    pub usage_details: Option<UsageDetails>,
}

#[derive(Debug, Serialize, Deserialize)]
struct UsageDetails {
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

fn bytes_to_typed_array(bytes: &[u8]) -> Result<JsValue> {
    let array = Uint8Array::new_with_length(bytes.len() as u32);
    array.copy_from(&bytes);
    Ok(JsValue::from(array))
}

fn typed_array_to_bytes(js_value: JsValue) -> Result<Vec<u8>> {
    Ok(js_value
        .dyn_into::<Uint8Array>()
        .map_err(|error| anyhow!("{:?}", error))?
        .to_vec())
}
