use anyhow::{anyhow, Result};
use async_trait::async_trait;
use cid::{
    multihash::{Code, MultihashDigest},
    Cid,
};
use libipld_core::{
    codec::{Codec, Decode, Encode},
    ipld::Ipld,
    raw::RawCodec,
};
use std::{
    collections::HashMap,
    io::Cursor,
    sync::{Arc, Mutex},
};

#[cfg(not(target_arch = "wasm32"))]
pub trait UcanStoreConditionalSend: Send {}

#[cfg(not(target_arch = "wasm32"))]
impl<U> UcanStoreConditionalSend for U where U: Send {}

#[cfg(target_arch = "wasm32")]
pub trait UcanStoreConditionalSend {}

#[cfg(target_arch = "wasm32")]
impl<U> UcanStoreConditionalSend for U {}

#[cfg(not(target_arch = "wasm32"))]
pub trait UcanStoreConditionalSendSync: Send + Sync {}

#[cfg(not(target_arch = "wasm32"))]
impl<U> UcanStoreConditionalSendSync for U where U: Send + Sync {}

#[cfg(target_arch = "wasm32")]
pub trait UcanStoreConditionalSendSync {}

#[cfg(target_arch = "wasm32")]
impl<U> UcanStoreConditionalSendSync for U {}

/// This trait is meant to be implemented by a storage backend suitable for
/// persisting UCAN tokens that may be referenced as proofs by other UCANs
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait UcanStore<C: Codec + Default = RawCodec>: UcanStoreConditionalSendSync {
    /// Read a value from the store by CID, returning a Result<Option<...>> that unwraps
    /// to None if no value is found, otherwise Some
    async fn read<T: Decode<C>>(&self, cid: &Cid) -> Result<Option<T>>;

    /// Write a value to the store, receiving a Result that wraps the values CID if the
    /// write was successful
    async fn write<T: Encode<C> + UcanStoreConditionalSend + core::fmt::Debug>(
        &mut self,
        token: T,
    ) -> Result<Cid>;
}

/// This trait is sugar over the UcanStore trait to add convenience methods
/// for the case of storing JWT-encoded UCAN strings using the 'raw' codec
/// which is the only combination strictly required by the UCAN spec
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait UcanJwtStore: UcanStore<RawCodec> {
    async fn require_token(&self, cid: &Cid) -> Result<String> {
        match self.read_token(cid).await? {
            Some(token) => Ok(token),
            None => Err(anyhow!("No token found for CID {}", cid.to_string())),
        }
    }

    async fn read_token(&self, cid: &Cid) -> Result<Option<String>> {
        let codec = RawCodec;

        if cid.codec() != u64::from(codec) {
            return Err(anyhow!(
                "Only 'raw' codec supported, but CID refers to {:#x}",
                cid.codec()
            ));
        }

        match self.read::<Ipld>(cid).await? {
            Some(Ipld::Bytes(bytes)) => Ok(Some(std::str::from_utf8(&bytes)?.to_string())),
            _ => Err(anyhow!("No UCAN was found for CID {:?}", cid)),
        }
    }

    async fn write_token(&mut self, token: &str) -> Result<Cid> {
        self.write(Ipld::Bytes(token.as_bytes().to_vec())).await
    }
}

impl<U> UcanJwtStore for U where U: UcanStore<RawCodec> {}

/// A basic in-memory store that implements UcanStore for the 'raw'
/// codec. This will serve for basic use cases and tests, but it is
/// recommended that a store that persists to disk be used in most
/// practical use cases.
#[derive(Clone, Default, Debug)]
pub struct MemoryStore {
    dags: Arc<Mutex<HashMap<Cid, Vec<u8>>>>,
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl UcanStore<RawCodec> for MemoryStore {
    async fn read<T: Decode<RawCodec>>(&self, cid: &Cid) -> Result<Option<T>> {
        let codec = RawCodec;
        let dags = self.dags.lock().map_err(|_| anyhow!("poisoned mutex!"))?;

        Ok(match dags.get(cid) {
            Some(bytes) => Some(T::decode(codec, &mut Cursor::new(bytes))?),
            None => None,
        })
    }

    async fn write<T: Encode<RawCodec> + UcanStoreConditionalSend + core::fmt::Debug>(
        &mut self,
        token: T,
    ) -> Result<Cid> {
        let codec = RawCodec;
        let block = codec.encode(&token)?;
        let cid = Cid::new_v1(codec.into(), Code::Blake3_256.digest(&block));

        let mut dags = self.dags.lock().map_err(|_| anyhow!("poisoned mutex!"))?;
        dags.insert(cid, block);

        Ok(cid)
    }
}
