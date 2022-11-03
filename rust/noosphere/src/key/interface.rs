use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ucan::crypto::KeyMaterial;

// #[cfg(not(target_arch = "wasm32"))]
// pub trait KeyStorageSendSync: Send + Sync {}

// #[cfg(not(target_arch = "wasm32"))]
// impl<S> KeyStorageSendSync for S where S: Send + Sync {}

// #[cfg(target_arch = "wasm32")]
// pub trait KeyStorageSend {}

/// A trait that represents access to arbitrary key storage backends.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait KeyStorage<K>
where
    K: KeyMaterial,
{
    /// Read a key by name from key storage.
    async fn read_key(&self, name: &str) -> Result<Option<K>>;
    /// Read a key by name from key storage, but return an error if no key is
    /// found by that name.
    async fn require_key(&self, name: &str) -> Result<K> {
        match self.read_key(name).await? {
            Some(key) => Ok(key),
            None => Err(anyhow!("No key named {} found!", name)),
        }
    }
    /// Create a key associated with the given name in key storage.
    async fn create_key(&self, name: &str) -> Result<K>;
}
