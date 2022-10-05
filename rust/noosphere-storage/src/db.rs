use anyhow::Result;
use async_trait::async_trait;
use cid::Cid;
use libipld_core::{
    codec::{Codec, Decode, Encode, References},
    ipld::Ipld,
    raw::RawCodec,
};
use std::fmt::Debug;
use ucan::store::{UcanStore, UcanStoreConditionalSend};

use crate::interface::{BlockStore, KeyValueStore, StorageProvider, Store};

#[cfg(not(target_arch = "wasm32"))]
pub trait SphereDbSendSync: Send + Sync {}

#[cfg(not(target_arch = "wasm32"))]
impl<T> SphereDbSendSync for T where T: Send + Sync {}

#[cfg(target_arch = "wasm32")]
pub trait SphereDbSendSync {}

#[cfg(target_arch = "wasm32")]
impl<T> SphereDbSendSync for T {}

pub const BLOCK_STORE: &str = "blocks";
pub const LINK_STORE: &str = "links";
pub const VERSION_STORE: &str = "versions";

#[derive(Clone)]
pub struct SphereDb<S>
where
    S: Store,
{
    block_store: S,
    link_store: S,
    version_store: S,
}

impl<S> SphereDb<S>
where
    S: Store,
{
    pub async fn new<P: StorageProvider<S>>(storage_provider: &P) -> Result<SphereDb<S>> {
        Ok(SphereDb {
            block_store: storage_provider.get_store(BLOCK_STORE).await?,
            link_store: storage_provider.get_store(LINK_STORE).await?,
            version_store: storage_provider.get_store(VERSION_STORE).await?,
        })
    }

    pub async fn set_version(&mut self, identity: &str, version: &Cid) -> Result<()> {
        self.version_store
            .set_key(identity.to_string(), version)
            .await
    }

    pub async fn get_version(&self, identity: &str) -> Result<Option<Cid>> {
        self.version_store.get_key(identity).await
    }

    pub async fn get_links(&self, cid: &Cid) -> Result<Option<Vec<Cid>>> {
        self.link_store.get_key(&cid.to_string()).await
    }

    pub fn to_block_store(&self) -> S {
        self.block_store.clone()
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S> BlockStore for SphereDb<S>
where
    S: Store,
{
    async fn put_links<C>(&mut self, cid: &Cid, block: &[u8]) -> Result<()>
    where
        C: Codec + Default,
        Ipld: References<C>,
    {
        let codec = C::default();
        let mut links = Vec::new();

        codec.references::<Ipld, _>(block, &mut links)?;

        self.link_store.set_key(&cid.to_string(), links).await?;

        Ok(())
    }

    async fn put_block(&mut self, cid: &cid::Cid, block: &[u8]) -> Result<()> {
        self.block_store.put_block(cid, block).await
    }

    async fn get_block(&self, cid: &cid::Cid) -> Result<Option<Vec<u8>>> {
        self.block_store.get_block(cid).await
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S> UcanStore for SphereDb<S>
where
    S: Store,
{
    async fn read<T: Decode<RawCodec>>(&self, cid: &Cid) -> Result<Option<T>> {
        self.get::<RawCodec, T>(cid).await
    }

    async fn write<T: Encode<RawCodec> + UcanStoreConditionalSend + Debug>(
        &mut self,
        token: T,
    ) -> Result<Cid> {
        self.put::<RawCodec, T>(token).await
    }
}

#[cfg(test)]
mod tests {

    use libipld_cbor::DagCborCodec;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

    use crate::{interface::BlockStore, memory::MemoryStorageProvider};

    use super::SphereDb;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    pub async fn it_stores_links_when_a_block_is_saved() {
        let storage_provider = MemoryStorageProvider::default();
        let mut db = SphereDb::new(&storage_provider).await.unwrap();

        let list1 = vec!["cats", "dogs", "pigeons"];
        let list2 = vec!["apples", "oranges", "starfruit"];

        let cid1 = db.save::<DagCborCodec, _>(&list1).await.unwrap();
        let cid2 = db.save::<DagCborCodec, _>(&list2).await.unwrap();

        let list3 = vec![cid1, cid2];

        let cid3 = db.save::<DagCborCodec, _>(&list3).await.unwrap();

        let links = db.get_links(&cid3).await.unwrap();

        assert_eq!(Some(list3), links);
    }
}
