use anyhow::Result;
use async_trait::async_trait;
use cid::Cid;
use core::fmt::Debug;
use libipld_core::{
    codec::{Decode, Encode},
    raw::RawCodec,
};
use ucan::store::{UcanStore as UcanStoreTrait, UcanStoreConditionalSend};

use crate::block::BlockStore;

pub struct UcanStore<S: BlockStore>(pub S);

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S: BlockStore> UcanStoreTrait<RawCodec> for UcanStore<S> {
    async fn read<T: Decode<RawCodec>>(&self, cid: &Cid) -> Result<Option<T>> {
        self.0.get::<RawCodec, T>(cid).await
    }

    async fn write<T: Encode<RawCodec> + UcanStoreConditionalSend + Debug>(
        &mut self,
        token: T,
    ) -> Result<Cid> {
        self.0.put::<RawCodec, T>(token).await
    }
}

impl<S: BlockStore> Clone for UcanStore<S> {
    fn clone(&self) -> Self {
        UcanStore(self.0.clone())
    }
}
