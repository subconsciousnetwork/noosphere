use std::rc::Rc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use noosphere_ucan_key_support::web_crypto::WebCryptoRsaKeyMaterial;
use rexie::{KeyRange, ObjectStore, Rexie, RexieBuilder, Store, Transaction, TransactionMode};
use std::sync::Arc;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::CryptoKey;

use super::KeyStorage;

/// An implementation of key storage backed by the Web Crypto and IndexedDB
/// APIs. This implementation is more secure than storing keys in clear text,
/// but doesn't strictly guarantee that a key is ultimately stored in some
/// kind of hardware-backed secure storage.
#[derive(Clone)]
pub struct WebCryptoKeyStorage {
    db: Rc<Rexie>,
}

pub const INDEXEDDB_STORAGE_VERSION: u32 = 1;
pub const STORE_NAME: &str = "keys";

impl WebCryptoKeyStorage {
    /// Initialize a new [WebCryptoKeyStorage] using the given name. This name
    /// will correspond to an underlying IndexedDB database name, so
    /// initializing the storage again using the same name will typically enable
    /// cross-session persistance.
    pub async fn new(db_name: &str) -> Result<Self> {
        Self::configure(INDEXEDDB_STORAGE_VERSION, db_name, &[STORE_NAME]).await
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

        Ok(WebCryptoKeyStorage { db: Rc::new(db) })
    }

    fn start_transaction(&self, mode: TransactionMode) -> Result<(Store, Transaction)> {
        let tx = self
            .db
            .transaction(&[STORE_NAME], mode)
            .map_err(|error| anyhow!("{:?}", error))?;
        let store = tx
            .store(STORE_NAME)
            .map_err(|error| anyhow!("{:?}", error))?;

        Ok((store, tx))
    }

    async fn finish_transaction(tx: Transaction) -> Result<()> {
        tx.done().await.map_err(|error| anyhow!("{:?}", error))?;
        Ok(())
    }

    async fn contains(key: &JsValue, store: &Store) -> Result<bool> {
        let count = store
            .count(Some(
                &KeyRange::only(key).map_err(|error| anyhow!("{:?}", error))?,
            ))
            .await
            .map_err(|error| anyhow!("{:?}", error))?;
        Ok(count > 0)
    }

    async fn read(key: &JsValue, store: &Store) -> Result<Option<CryptoKey>> {
        Ok(match Self::contains(&key, &store).await? {
            true => Some(
                store
                    .get(&key)
                    .await
                    .map_err(|error| anyhow!("{:?}", error))?
                    .dyn_into::<CryptoKey>()
                    .map_err(|error| anyhow!("{:?}", error))?,
            ),
            false => None,
        })
    }
}

#[async_trait(?Send)]
impl KeyStorage<Arc<WebCryptoRsaKeyMaterial>> for WebCryptoKeyStorage {
    async fn read_key(&self, name: &str) -> Result<Option<Arc<WebCryptoRsaKeyMaterial>>> {
        let (store, tx) = self.start_transaction(TransactionMode::ReadWrite)?;

        let private_key_name = JsValue::from_str(&format!("{}/private", name));
        let public_key_name = JsValue::from_str(&format!("{}/public", name));

        let private_key = match WebCryptoKeyStorage::read(&private_key_name, &store).await? {
            Some(key) => key,
            None => return Ok(None),
        };
        let public_key = match WebCryptoKeyStorage::read(&public_key_name, &store).await? {
            Some(key) => key,
            None => return Ok(None),
        };

        WebCryptoKeyStorage::finish_transaction(tx).await?;

        Ok(Some(Arc::new(WebCryptoRsaKeyMaterial(
            public_key,
            Some(private_key),
        ))))
    }

    async fn create_key(&self, name: &str) -> Result<Arc<WebCryptoRsaKeyMaterial>> {
        let key_material = WebCryptoRsaKeyMaterial::generate(None).await?;
        let (store, tx) = self.start_transaction(TransactionMode::ReadWrite)?;

        let private_key_name = JsValue::from_str(&format!("{}/private", name));
        let public_key_name = JsValue::from_str(&format!("{}/public", name));

        if WebCryptoKeyStorage::contains(&private_key_name, &store).await? {
            return Err(anyhow!("Key name already exists!"));
        }

        let private_key = key_material
            .1
            .as_ref()
            .ok_or_else(|| anyhow!("No private key generated!"))?;

        let public_key = &key_material.0;

        store
            .put(&JsValue::from(private_key), Some(&private_key_name))
            .await
            .map_err(|error| anyhow!("{:?}", error))?;

        store
            .put(&JsValue::from(public_key), Some(&public_key_name))
            .await
            .map_err(|error| anyhow!("{:?}", error))?;

        WebCryptoKeyStorage::finish_transaction(tx).await?;

        Ok(Arc::new(key_material))
    }
}

#[cfg(test)]
mod tests {
    use crate::key::KeyStorage;

    use super::WebCryptoKeyStorage;
    use noosphere_ucan::crypto::KeyMaterial;

    use wasm_bindgen_test::wasm_bindgen_test;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn it_can_create_and_read_a_key() {
        let db_name: String = witty_phrase_generator::WPGen::new()
            .with_words(3)
            .unwrap()
            .into_iter()
            .map(|word| String::from(word))
            .collect();

        let created_key = {
            let key_storage = WebCryptoKeyStorage::new(&db_name).await.unwrap();
            key_storage.create_key("foo").await.unwrap()
        };

        let retrieved_key = {
            let key_storage = WebCryptoKeyStorage::new(&db_name).await.unwrap();
            key_storage.require_key("foo").await.unwrap()
        };

        assert_eq!(
            created_key.get_did().await.unwrap(),
            retrieved_key.get_did().await.unwrap()
        )
    }
}
