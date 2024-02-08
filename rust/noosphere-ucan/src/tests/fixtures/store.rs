use crate::store::{UcanStore, UcanStoreConditionalSend};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use cid::{
    multihash::{Code, MultihashDigest},
    Cid,
};
use libipld_core::{
    codec::{Codec, Decode, Encode},
    raw::RawCodec,
};
use std::{
    collections::HashMap,
    io::Cursor,
    sync::{Arc, Mutex},
};

#[derive(Clone, Default, Debug)]
pub struct Blake2bMemoryStore {
    dags: Arc<Mutex<HashMap<Cid, Vec<u8>>>>,
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl UcanStore<RawCodec> for Blake2bMemoryStore {
    async fn read<T: Decode<RawCodec>>(&self, cid: &Cid) -> Result<Option<T>> {
        let dags = self.dags.lock().map_err(|_| anyhow!("poisoned mutex!"))?;

        Ok(match dags.get(cid) {
            Some(bytes) => Some(T::decode(RawCodec, &mut Cursor::new(bytes))?),
            None => None,
        })
    }

    async fn write<T: Encode<RawCodec> + UcanStoreConditionalSend + core::fmt::Debug>(
        &mut self,
        token: T,
    ) -> Result<Cid> {
        let codec = RawCodec;
        let block = codec.encode(&token)?;
        let cid = Cid::new_v1(codec.into(), Code::Blake2b256.digest(&block));

        let mut dags = self.dags.lock().map_err(|_| anyhow!("poisoned mutex!"))?;
        dags.insert(cid, block);

        Ok(cid)
    }
}
