use std::io::Cursor;

use anyhow::Result;
use async_trait::async_trait;
use cid::Cid;
use libipld_cbor::DagCborCodec;
use libipld_core::{
    codec::{Codec, Decode},
    ipld::Ipld,
    serde::{from_ipld, to_ipld},
};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    block::BlockStore,
    key_value::{KeyValueStore, KeyValueStoreSend},
};

#[cfg(not(target_arch = "wasm32"))]
pub trait StoreConditionalSendSync: Send + Sync {}

#[cfg(not(target_arch = "wasm32"))]
impl<S> StoreConditionalSendSync for S where S: Send + Sync {}

#[cfg(target_arch = "wasm32")]
pub trait StoreConditionalSendSync {}

#[cfg(target_arch = "wasm32")]
impl<S> StoreConditionalSendSync for S {}

/// A primitive interface for storage backends. A storage backend does not
/// necessarily need to implement this trait to be used in Noosphere, but if it
/// does it automatically benefits from trait implementations for [BlockStore]
/// and [KeyValueStore], making a single [Store] implementation into a universal
/// backend.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait Store: Clone + StoreConditionalSendSync {
    /// Read the bytes stored against a given key
    async fn read(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;

    /// Writes bytes to local storage against a given key, and returns the previous
    /// value stored against that key if any
    async fn write(&mut self, key: &[u8], bytes: &[u8]) -> Result<Option<Vec<u8>>>;

    /// Remove a value given a key, returning the removed value if any
    async fn remove(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>>;

    /// Flushes pending writes if there are any
    async fn flush(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S> BlockStore for S
where
    S: Store,
{
    async fn put_block(&mut self, cid: &Cid, block: &[u8]) -> Result<()> {
        self.write(&cid.to_bytes(), block).await?;
        Ok(())
    }

    async fn get_block(&self, cid: &Cid) -> Result<Option<Vec<u8>>> {
        self.read(&cid.to_bytes()).await
    }

    async fn flush(&self) -> Result<()> {
        Store::flush(self).await
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S> KeyValueStore for S
where
    S: Store,
{
    async fn set_key<K, V>(&mut self, key: K, value: V) -> Result<()>
    where
        K: AsRef<[u8]> + KeyValueStoreSend,
        V: Serialize + KeyValueStoreSend,
    {
        let ipld = to_ipld(value)?;
        let codec = DagCborCodec;
        let cbor = codec.encode(&ipld)?;
        let key_bytes = K::as_ref(&key);
        self.write(key_bytes, &cbor).await?;
        Ok(())
    }

    async fn unset_key<K>(&mut self, key: K) -> Result<()>
    where
        K: AsRef<[u8]> + KeyValueStoreSend,
    {
        let key_bytes = K::as_ref(&key);
        self.remove(key_bytes).await?;
        Ok(())
    }

    async fn get_key<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: AsRef<[u8]> + KeyValueStoreSend,
        V: DeserializeOwned + KeyValueStoreSend,
    {
        let key_bytes = K::as_ref(&key);
        Ok(match self.read(key_bytes).await? {
            Some(bytes) => Some(from_ipld(Ipld::decode(
                DagCborCodec,
                &mut Cursor::new(bytes),
            )?)?),
            None => None,
        })
    }

    async fn flush(&self) -> Result<()> {
        Store::flush(self).await
    }
}
