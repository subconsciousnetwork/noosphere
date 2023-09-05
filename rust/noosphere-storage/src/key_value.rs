use std::fmt::Display;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use noosphere_common::{ConditionalSend, ConditionalSync};
use serde::{de::DeserializeOwned, Serialize};

/// A [KeyValueStore] is a construct that is suitable for persisting generic
/// key/value data to a storage backend.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait KeyValueStore: Clone + ConditionalSync {
    /// Given some key that can be realized as bytes, persist a serializable
    /// value to storage so that it can later be retrieved by that key
    async fn set_key<K, V>(&mut self, key: K, value: V) -> Result<()>
    where
        K: AsRef<[u8]> + ConditionalSend,
        V: Serialize + ConditionalSend;

    /// Given some key that can be realized as bytes, retrieve some data that
    /// can be deserialized as the intended data structure
    async fn get_key<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: AsRef<[u8]> + ConditionalSend,
        V: DeserializeOwned + ConditionalSend;

    /// Given some key that can be realized as bytes, unset the value stored
    /// against that key (if any)
    async fn unset_key<K>(&mut self, key: K) -> Result<()>
    where
        K: AsRef<[u8]> + ConditionalSend;

    /// Same as get_key, but returns an error if no value is found to be stored
    /// against the key
    async fn require_key<K, V>(&self, key: K) -> Result<V>
    where
        K: AsRef<[u8]> + ConditionalSend + Display,
        V: DeserializeOwned + ConditionalSend,
    {
        let required = key.to_string();

        match self.get_key(key).await? {
            Some(value) => Ok(value),
            None => Err(anyhow!("No value found for '{required}'")),
        }
    }

    /// Flushes pending writes if there are any
    async fn flush(&self) -> Result<()> {
        Ok(())
    }
}
