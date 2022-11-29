use std::fmt::Display;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};

#[cfg(not(target_arch = "wasm32"))]
pub trait KeyValueSendSync: Send + Sync {}

#[cfg(not(target_arch = "wasm32"))]
impl<T> KeyValueSendSync for T where T: Send + Sync {}

#[cfg(target_arch = "wasm32")]
pub trait KeyValueSendSync {}

#[cfg(target_arch = "wasm32")]
impl<T> KeyValueSendSync for T {}

#[cfg(not(target_arch = "wasm32"))]
pub trait KeyValueStoreSend: Send {}

#[cfg(not(target_arch = "wasm32"))]
impl<T> KeyValueStoreSend for T where T: Send {}

#[cfg(target_arch = "wasm32")]
pub trait KeyValueStoreSend {}

#[cfg(target_arch = "wasm32")]
impl<T> KeyValueStoreSend for T {}

/// A [KeyValueStore] is a construct that is suitable for persisting generic
/// key/value data to a storage backend.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait KeyValueStore: Clone + KeyValueSendSync {
    /// Given some key that can be realized as bytes, persist a serializable
    /// value to storage so that it can later be retrieved by that key
    async fn set_key<K, V>(&mut self, key: K, value: V) -> Result<()>
    where
        K: AsRef<[u8]> + KeyValueStoreSend,
        V: Serialize + KeyValueStoreSend;

    /// Given some key that can be realized as bytes, retrieve some data that
    /// can be deserialized as the intended data structure
    async fn get_key<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: AsRef<[u8]> + KeyValueStoreSend,
        V: DeserializeOwned + KeyValueStoreSend;

    /// Given some key that can be realized as bytes, unset the value stored
    /// against that key (if any)
    async fn unset_key<K>(&mut self, key: K) -> Result<()>
    where
        K: AsRef<[u8]> + KeyValueStoreSend;

    /// Same as get_key, but returns an error if no value is found to be stored
    /// against the key
    async fn require_key<K, V>(&self, key: K) -> Result<V>
    where
        K: AsRef<[u8]> + KeyValueStoreSend + Display,
        V: DeserializeOwned + KeyValueStoreSend,
    {
        let required = key.to_string();

        match self.get_key(key).await? {
            Some(value) => Ok(value),
            None => Err(anyhow!("No value found for '{required}'")),
        }
    }
}
