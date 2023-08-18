use crate::{
    db::{SCRATCH_STORE, SPHERE_DB_STORE_NAMES},
    storage::Storage,
};
use crate::{store::Store, Scratch};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use js_sys::Uint8Array;
use rexie::{
    KeyRange, ObjectStore, Rexie, RexieBuilder, Store as IdbStore, Transaction, TransactionMode,
};
use std::{fmt::Debug, rc::Rc};
use wasm_bindgen::{JsCast, JsValue};

pub const INDEXEDDB_STORAGE_VERSION: u32 = 2;

#[derive(Clone)]
pub struct WebStorage {
    db: Rc<Rexie>,
}

impl Debug for WebStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebStorage").finish()
    }
}

impl WebStorage {
    pub async fn new(db_name: &str) -> Result<Self> {
        let storage =
            Self::configure(INDEXEDDB_STORAGE_VERSION, db_name, SPHERE_DB_STORE_NAMES).await?;
        WebStorage::clear_scratch_storage(storage.db.clone()).await?;
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

    /// We cannot dynamically create tables with IndexedDb, so we create
    /// a generic `SCRATCH_STORE` table that all scratch stores use,
    /// each partitioning keys by a random key prefix. In lieu of
    /// removing these values on [TempWebStore] drop (no async drop), we
    /// clear out the scratch storage on [WebStorage] instantiation.
    async fn clear_scratch_storage(db: Rc<Rexie>) -> Result<()> {
        let scratch = WebStore {
            db,
            store_name: SCRATCH_STORE.to_owned(),
        };
        let (store, tx) = scratch.start_transaction(TransactionMode::ReadWrite)?;
        store
            .clear()
            .await
            .map_err(|error| anyhow!("{:?}", error))?;
        WebStore::finish_transaction(tx).await?;
        Ok(())
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

#[async_trait(?Send)]
impl Scratch for WebStorage {
    type ScratchStore = TempWebStore;

    async fn get_scratch_store(&self) -> Result<Self::ScratchStore> {
        Ok(TempWebStore::new(self.db.clone()))
    }
}

#[derive(Clone)]
pub struct WebStore {
    db: Rc<Rexie>,
    store_name: String,
}

impl WebStore {
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

#[derive(Clone)]
/// A [WebStore] that does not persist data after dropping.
/// Can be created from [WebStorage]'s [Scratch] implementation.
pub struct TempWebStore {
    store: WebStore,
    partition_name: Vec<u8>,
}

impl TempWebStore {
    pub(crate) fn new(db: Rc<Rexie>) -> Self {
        let store = WebStore {
            db,
            store_name: SCRATCH_STORE.to_owned(),
        };
        let partition_name = format!("temp-web-store-{}/", rand::random::<u32>()).into();
        TempWebStore {
            store,
            partition_name,
        }
    }

    fn partition_key(&self, key: &[u8]) -> Vec<u8> {
        vec![self.partition_name.clone(), key.to_owned()].concat()
    }
}

#[async_trait(?Send)]
impl Store for TempWebStore {
    async fn read(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        self.store.read(&self.partition_key(key)).await
    }

    async fn write(&mut self, key: &[u8], bytes: &[u8]) -> Result<Option<Vec<u8>>> {
        self.store.write(&self.partition_key(key), bytes).await
    }

    async fn remove(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        self.store.remove(&self.partition_key(key)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key_value::KeyValueStore;
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
    wasm_bindgen_test_configure!(run_in_browser);

    #[derive(Clone)]
    pub struct WebStorageV1 {
        db: Rc<Rexie>,
    }

    impl WebStorageV1 {
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

            Ok(WebStorageV1 { db: Rc::new(db) })
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

    #[wasm_bindgen_test]
    async fn it_can_upgrade_from_v1() -> Result<()> {
        let key = String::from("foo");
        let value = String::from("bar");
        {
            let storage_v1 = WebStorageV1::new("v1_test").await?;
            let mut store_v1 = storage_v1.get_store("links").await?;
            store_v1.set_key(&key, &value).await?;
        }

        let storage_v2 = WebStorage::new("v1_test").await?;
        let store_v2 = storage_v2.get_store("links").await?;
        assert_eq!(store_v2.get_key::<_, String>(&key).await?.unwrap(), value);
        Ok(())
    }
}
