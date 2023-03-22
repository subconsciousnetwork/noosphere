use std::io::Cursor;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use cid::{
    multihash::{Code, MultihashDigest},
    Cid,
};
use libipld_core::{
    codec::{Codec, Decode, Encode, References},
    ipld::Ipld,
    serde::{from_ipld, to_ipld},
};
use serde::{de::DeserializeOwned, Serialize};

#[cfg(doc)]
use serde::Deserialize;

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

/// An interface for storage backends that are suitable for storing blocks. A
/// block is a chunk of bytes that can be addressed by a
/// [CID](https://docs.ipfs.tech/concepts/content-addressing/#identifier-formats).
/// Any backend that implements this trait should be able to reliably store and
/// retrieve blocks given a [Cid].
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait BlockStore: Clone + BlockStoreSendSync {
    /// Given a CID and a block, store the links (any [Cid] that is part of the
    /// encoded data) in a suitable location for later retrieval. This method is
    /// optional, and its default implementation is a no-op. It should be
    /// implemented when possible to enable optimized traversal of a DAG given
    /// its root.
    #[allow(unused_variables)]
    async fn put_links<C>(&mut self, cid: &Cid, block: &[u8]) -> Result<()>
    where
        C: Codec + Default,
        Ipld: References<C>,
    {
        Ok(())
    }

    /// Given a block and its [Cid], persist the block in storage.
    async fn put_block(&mut self, cid: &Cid, block: &[u8]) -> Result<()>;

    /// Given the [Cid] of a block, retrieve the block bytes from storage.
    async fn get_block(&self, cid: &Cid) -> Result<Option<Vec<u8>>>;

    /// Given some data structure that implements [Encode] for a given [Codec],
    /// encode it as a block and persist it to storage for later retrieval by
    /// [Cid].
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

    /// Given the [Cid] of a block that refers to a type that implements
    /// [Decode] for some [Codec], retrieve the block, decode it as the type and
    /// return the result.
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

    /// Given some data structure that implements [Serialize], convert it to an
    /// [IPLD](https://ipld.io/docs/)-compatible representation, encode it as a
    /// block with the desired [Codec] and persist it to the storage backend by
    /// its [Cid]
    async fn save<C, T>(&mut self, data: T) -> Result<Cid>
    where
        C: Codec + Default,
        T: Serialize + BlockStoreSend,
        Ipld: Encode<C> + References<C>,
    {
        self.put::<C, Ipld>(to_ipld(data)?).await
    }

    /// Given a [Cid] that refers to some data structure that implements
    /// [Deserialize], read the block bytes from storage, decode it as
    /// [IPLD](https://ipld.io/docs/) using the specified [Codec] and and
    /// deserialize it to the intended data structure, returning the result.
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

    /// Same as load, but returns an error if no block is found locally for the
    /// given [Cid]
    async fn require_block(&self, cid: &Cid) -> Result<Vec<u8>> {
        match self.get_block(cid).await? {
            Some(block) => Ok(block),
            None => Err(anyhow!("Block {cid} was required but not found")),
        }
    }

    /// Flushes pending writes if there are any
    async fn flush(&self) -> Result<()> {
        Ok(())
    }
}
