use std::io::Cursor;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use cid::{
    multihash::{Code, MultihashDigest},
    Cid,
};
use libipld_cbor::DagCborCodec;
use libipld_core::{
    codec::References,
    serde::{from_ipld, to_ipld},
};
use libipld_core::{
    codec::{Codec, Decode, Encode},
    ipld::Ipld,
};
use serde::{de::DeserializeOwned, Serialize};

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait StorageProvider<S: Store> {
    async fn get_store(&self, name: &str) -> Result<S>;
}

#[cfg(not(target_arch = "wasm32"))]
pub trait StoreConditionalSendSync: Send + Sync {}

#[cfg(not(target_arch = "wasm32"))]
impl<S> StoreConditionalSendSync for S where S: Send + Sync {}

#[cfg(target_arch = "wasm32")]
pub trait StoreConditionalSendSync {}

#[cfg(target_arch = "wasm32")]
impl<S> StoreConditionalSendSync for S {}

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

#[cfg(not(target_arch = "wasm32"))]
pub trait BlockStoreSendSync: Send + Sync {}

#[cfg(not(target_arch = "wasm32"))]
impl<T> BlockStoreSendSync for T where T: Send + Sync {}

#[cfg(target_arch = "wasm32")]
pub trait BlockStoreSendSync {}

#[cfg(target_arch = "wasm32")]
impl<T> BlockStoreSendSync for T {}

#[cfg(not(target_arch = "wasm32"))]
pub trait BlockStoreSend: Send {}

#[cfg(not(target_arch = "wasm32"))]
impl<T> BlockStoreSend for T where T: Send {}

#[cfg(target_arch = "wasm32")]
pub trait BlockStoreSend {}

#[cfg(target_arch = "wasm32")]
impl<T> BlockStoreSend for T {}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait BlockStore: Clone + BlockStoreSendSync {
    #[allow(unused_variables)]
    async fn put_links<C>(&mut self, cid: &Cid, block: &[u8]) -> Result<()>
    where
        C: Codec + Default,
        Ipld: References<C>,
    {
        Ok(())
    }

    async fn put_block(&mut self, cid: &Cid, block: &[u8]) -> Result<()>;

    async fn get_block(&self, cid: &Cid) -> Result<Option<Vec<u8>>>;

    async fn put<C, T>(&mut self, data: T) -> Result<Cid>
    where
        C: Codec + Default,
        T: Encode<C> + BlockStoreSend,
        Ipld: References<C>,
    {
        let codec = C::default();
        let block = codec.encode(&data)?;
        let cid = Cid::new_v1(codec.into(), Code::Blake2b256.digest(&block));

        self.put_block(&cid, &block).await?;
        self.put_links::<C>(&cid, &block).await?;

        Ok(cid)
    }

    async fn get<C, T>(&self, cid: &Cid) -> Result<Option<T>>
    where
        C: Codec + Default,
        T: Decode<C>,
    {
        let codec = C::default();
        let block = self.get_block(cid).await?;

        Ok(match block {
            Some(bytes) => Some(T::decode(codec, &mut Cursor::new(bytes))?),
            None => None,
        })
    }

    async fn save<C, T>(&mut self, data: T) -> Result<Cid>
    where
        C: Codec + Default,
        T: Serialize + BlockStoreSend,
        Ipld: Encode<C> + References<C>,
    {
        self.put::<C, Ipld>(to_ipld(data)?).await
    }

    async fn load<C, T>(&self, cid: &Cid) -> Result<T>
    where
        C: Codec + Default,
        T: DeserializeOwned + BlockStoreSend,
        u64: From<C>,
        Ipld: Decode<C>,
    {
        let codec = u64::from(C::default());

        if cid.codec() != codec {
            return Err(anyhow!(
                "Incorrect codec; expected {}, but CID refers to {}",
                codec,
                cid.codec()
            ));
        }

        Ok(match self.get::<C, Ipld>(cid).await? {
            Some(ipld) => from_ipld(ipld)?,
            None => return Err(anyhow!("No block found for {}", cid)),
        })
    }

    async fn require_block(&self, cid: &Cid) -> Result<Vec<u8>> {
        match self.get_block(cid).await? {
            Some(block) => Ok(block),
            None => Err(anyhow!("Block {cid} was required but not found")),
        }
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
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait KeyValueStore: Store {
    async fn set_key<K, V>(&mut self, key: K, value: V) -> Result<()>
    where
        K: AsRef<[u8]> + BlockStoreSend,
        V: Serialize + BlockStoreSend,
    {
        let ipld = to_ipld(value)?;
        let codec = DagCborCodec;
        let cbor = codec.encode(&ipld)?;
        let key_bytes = K::as_ref(&key);
        self.write(key_bytes, &cbor).await?;
        Ok(())
    }

    async fn get_key<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: AsRef<[u8]> + BlockStoreSend,
        V: DeserializeOwned + BlockStoreSend,
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
}

impl<S> KeyValueStore for S where S: Store {}
