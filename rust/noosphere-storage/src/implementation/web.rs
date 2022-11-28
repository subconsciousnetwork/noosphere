use crate::store::Store;
use crate::{db::SPHERE_DB_STORE_NAMES, storage::Storage};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use js_sys::Uint8Array;
use rexie::{
    KeyRange, ObjectStore, Rexie, RexieBuilder, Store as IdbStore, Transaction, TransactionMode,
};
use std::rc::Rc;
use wasm_bindgen::{JsCast, JsValue};

pub const INDEXEDDB_STORAGE_VERSION: u32 = 1;

#[derive(Clone)]
pub struct WebStorage {
    db: Rc<Rexie>,
}

impl WebStorage {
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

        Ok(WebStorage { db: Rc::new(db) })
    }

    async fn get_store(&self, name: &str) -> Result<WebStore> {
        if self
            .db
            .store_names()
            .iter()
            .find(|val| val.as_str() == name)
            .is_none()
        {
            return Err(anyhow!("No such store named {}", name));
        }

        Ok(WebStore {
            db: self.db.clone(),
            store_name: name.to_string(),
        })
    }
}

#[async_trait(?Send)]
impl Storage for WebStorage {
    type BlockStore = WebStore;

    type KeyValueStore = WebStore;

    async fn get_block_store(&self, name: &str) -> Result<Self::BlockStore> {
        self.get_store(name).await
    }

    async fn get_key_value_store(&self, name: &str) -> Result<Self::KeyValueStore> {
        self.get_store(name).await
    }
}

#[derive(Clone)]
pub struct WebStore {
    db: Rc<Rexie>,
    store_name: String,
}

impl WebStore {
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
        Ok(match WebStore::contains(&key, &store).await? {
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
impl Store for WebStore {
    async fn read(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let (store, tx) = self.start_transaction(TransactionMode::ReadOnly)?;
        let key = WebStore::bytes_to_typed_array(key)?;

        let maybe_dag = WebStore::read(&key, &store).await?;

        WebStore::finish_transaction(tx).await?;

        Ok(maybe_dag)
    }

    async fn write(&mut self, key: &[u8], bytes: &[u8]) -> Result<Option<Vec<u8>>> {
        let (store, tx) = self.start_transaction(TransactionMode::ReadWrite)?;

        let key = WebStore::bytes_to_typed_array(key)?;
        let value = WebStore::bytes_to_typed_array(bytes)?;

        let old_bytes = WebStore::read(&key, &store).await?;

        store
            .put(&value, Some(&key))
            .await
            .map_err(|error| anyhow!("{:?}", error))?;

        WebStore::finish_transaction(tx).await?;

        Ok(old_bytes)
    }

    async fn remove(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let (store, tx) = self.start_transaction(TransactionMode::ReadWrite)?;

        let key = WebStore::bytes_to_typed_array(key)?;

        let old_value = WebStore::read(&key, &store).await?;

        store
            .delete(&key)
            .await
            .map_err(|error| anyhow!("{:?}", error))?;

        WebStore::finish_transaction(tx).await?;

        Ok(old_value)
    }
}
