use anyhow::{anyhow, Result};
use async_trait::async_trait;
use cid::Cid;
use libipld_cbor::DagCborCodec;
use libipld_core::{
    codec::{Codec, Decode, Encode, References},
    ipld::Ipld,
    raw::RawCodec,
};
use noosphere_common::ConditionalSend;
use serde::{de::DeserializeOwned, Serialize};
use std::future::Future;
use std::{collections::BTreeSet, fmt::Debug};
use tokio_stream::Stream;
use ucan::store::UcanStore;

use crate::{BlockStore, EphemeralStorage, EphemeralStore, KeyValueStore, MemoryStore, Storage};

use async_stream::try_stream;

pub const BLOCK_STORE: &str = "blocks";
pub const LINK_STORE: &str = "links";
pub const VERSION_STORE: &str = "versions";
pub const METADATA_STORE: &str = "metadata";
pub const EPHEMERAL_STORE: &str = "ephemeral";

pub const SPHERE_DB_STORE_NAMES: &[&str] = &[
    BLOCK_STORE,
    LINK_STORE,
    VERSION_STORE,
    METADATA_STORE,
    EPHEMERAL_STORE,
];

/// A [SphereDb] is a high-level storage primitive for Noosphere's APIs. It
/// takes a [Storage] and implements [BlockStore] and [KeyValueStore],
/// orchestrating writes so that as blocks are stored, links are also extracted
/// and tracked separately, and also hosting metadata information such as sphere
/// version records and other purely local configuration
#[derive(Clone, Debug)]
pub struct SphereDb<S>
where
    S: Storage,
{
    block_store: S::BlockStore,
    link_store: S::KeyValueStore,
    version_store: S::KeyValueStore,
    metadata_store: S::KeyValueStore,
    storage: S,
}

impl<S> SphereDb<S>
where
    S: Storage,
{
    pub async fn new(storage: S) -> Result<SphereDb<S>> {
        Ok(SphereDb {
            block_store: storage.get_block_store(BLOCK_STORE).await?,
            link_store: storage.get_key_value_store(LINK_STORE).await?,
            version_store: storage.get_key_value_store(VERSION_STORE).await?,
            metadata_store: storage.get_key_value_store(METADATA_STORE).await?,
            storage,
        })
    }

    /// Given a [MemoryStore], store copies of all the blocks found within in
    /// the storage that backs this [SphereDb].
    pub async fn persist(&mut self, memory_store: &MemoryStore) -> Result<()> {
        let cids = memory_store.get_stored_cids().await;

        for cid in &cids {
            let block = memory_store.require_block(cid).await?;

            self.put_block(cid, &block).await?;

            match cid.codec() {
                codec_id if codec_id == u64::from(DagCborCodec) => {
                    self.put_links::<DagCborCodec>(cid, &block).await?;
                }
                codec_id if codec_id == u64::from(RawCodec) => {
                    self.put_links::<RawCodec>(cid, &block).await?;
                }
                codec_id => warn!("Unrecognized codec {}; skipping...", codec_id),
            }
        }
        Ok(())
    }

    /// Record the tip of a local sphere lineage as a [Cid]
    pub async fn set_version(&mut self, identity: &str, version: &Cid) -> Result<()> {
        self.version_store
            .set_key(identity.to_string(), version)
            .await
    }

    /// Get the most recently recorded tip of a local sphere lineage
    pub async fn get_version(&self, identity: &str) -> Result<Option<Cid>> {
        self.version_store.get_key(identity).await
    }

    /// Manually flush all pending writes to the underlying [Storage]
    pub async fn flush(&self) -> Result<()> {
        let (block_store_result, link_store_result, version_store_result, metadata_store_result) = tokio::join!(
            self.block_store.flush(),
            self.link_store.flush(),
            self.version_store.flush(),
            self.metadata_store.flush()
        );

        let results = vec![
            ("block", block_store_result),
            ("link", link_store_result),
            ("version", version_store_result),
            ("metadata", metadata_store_result),
        ];

        for (store_kind, result) in results {
            if let Err(error) = result {
                warn!("Failed to flush {} store: {:?}", store_kind, error);
            }
        }

        Ok(())
    }

    /// Get the most recently recorded tip of a local sphere lineage, returning
    /// an error if no version has ever been recorded
    pub async fn require_version(&self, identity: &str) -> Result<Cid> {
        self.version_store
            .get_key(identity)
            .await?
            .ok_or_else(|| anyhow!("No version was found for sphere {}", identity))
    }

    /// Get all links referenced by a block given its [Cid]
    pub async fn get_block_links(&self, cid: &Cid) -> Result<Option<Vec<Cid>>> {
        self.link_store.get_key(&cid.to_string()).await
    }

    /// Given a [Cid] root and a predicate function, stream all links that are
    /// referenced by the root or its descendants (recursively). The predicate
    /// function is called with each [Cid] before it is yielded by the stream.
    /// If the predicate returns true, the [Cid] is yielded and its referenced
    /// links are queued to be yielded later by the stream. If the predicate
    /// returns false, the [Cid] is skipped and by extension so are its
    /// referenced links.
    pub fn query_links<'a, F, P>(
        &'a self,
        cid: &'a Cid,
        predicate: P,
    ) -> impl Stream<Item = Result<Cid>> + 'a
    where
        F: Future<Output = Result<bool>>,
        P: Fn(&Cid) -> F + Send + Sync + 'static,
    {
        try_stream! {
            let mut visited_links = BTreeSet::new();
            let mut remaining_links = vec![*cid];

            while let Some(cid) = remaining_links.pop() {
                if visited_links.contains(&cid) {
                    continue;
                }

                if predicate(&cid).await? {
                    if let Some(mut links) = self.get_block_links(&cid).await? {
                        remaining_links.append(&mut links);
                    }

                    yield cid;
                }

                visited_links.insert(cid);
            }
        }
    }

    /// Stream all links that are referenced from the given root [Cid] or its
    /// DAG descendants (recursively).
    pub fn stream_links<'a>(&'a self, cid: &'a Cid) -> impl Stream<Item = Result<Cid>> + 'a {
        try_stream! {
            for await cid in self.query_links(cid, |_| async {Ok(true)}) {
                yield cid?;
            }
        }
    }

    /// Stream all the blocks in the DAG starting at the given root [Cid].
    pub fn stream_blocks<'a>(
        &'a self,
        cid: &'a Cid,
    ) -> impl Stream<Item = Result<(Cid, Vec<u8>)>> + 'a {
        try_stream! {
            for await cid in self.stream_links(cid) {
                let cid = cid?;
                if let Some(block) = self.block_store.get_block(&cid).await? {
                    yield (cid, block);
                }
            }
        }
    }

    /// Get an owned copy of the underlying primitive [BlockStore] for this
    /// [SphereDb]
    pub fn to_block_store(&self) -> S::BlockStore {
        self.block_store.clone()
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S> BlockStore for SphereDb<S>
where
    S: Storage,
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
impl<S> KeyValueStore for SphereDb<S>
where
    S: Storage,
{
    async fn set_key<K, V>(&mut self, key: K, value: V) -> Result<()>
    where
        K: AsRef<[u8]> + ConditionalSend,
        V: Serialize + ConditionalSend,
    {
        self.metadata_store.set_key(key, value).await
    }

    async fn unset_key<K>(&mut self, key: K) -> Result<()>
    where
        K: AsRef<[u8]> + ConditionalSend,
    {
        self.metadata_store.unset_key(key).await
    }

    async fn get_key<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: AsRef<[u8]> + ConditionalSend,
        V: DeserializeOwned + ConditionalSend,
    {
        self.metadata_store.get_key(key).await
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S> UcanStore for SphereDb<S>
where
    S: Storage,
{
    async fn read<T: Decode<RawCodec>>(&self, cid: &Cid) -> Result<Option<T>> {
        self.get::<RawCodec, T>(cid).await
    }

    async fn write<T: Encode<RawCodec> + ConditionalSend + Debug>(
        &mut self,
        token: T,
    ) -> Result<Cid> {
        self.put::<RawCodec, T>(token).await
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S> EphemeralStorage for SphereDb<S>
where
    S: Storage,
{
    type EphemeralStoreType = <S as EphemeralStorage>::EphemeralStoreType;

    async fn get_ephemeral_store(&self) -> Result<EphemeralStore<Self::EphemeralStoreType>> {
        self.storage.get_ephemeral_store().await
    }
}

#[cfg(test)]
mod tests {

    use libipld_cbor::DagCborCodec;
    use libipld_core::{ipld::Ipld, raw::RawCodec};
    use ucan::store::UcanJwtStore;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

    use crate::{block_encode, derive_cid, BlockStore, MemoryStorage, SphereDb};

    use tokio_stream::StreamExt;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    pub async fn it_stores_links_when_a_block_is_saved() {
        let storage_provider = MemoryStorage::default();
        let mut db = SphereDb::new(storage_provider).await.unwrap();

        let list1 = vec!["cats", "dogs", "pigeons"];
        let list2 = vec!["apples", "oranges", "starfruit"];

        let cid1 = db.save::<DagCborCodec, _>(&list1).await.unwrap();
        let cid2 = db.save::<DagCborCodec, _>(&list2).await.unwrap();

        let list3 = vec![cid1, cid2];

        let cid3 = db.save::<DagCborCodec, _>(&list3).await.unwrap();

        let links = db.get_block_links(&cid3).await.unwrap();

        assert_eq!(Some(list3), links);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    pub async fn it_can_stream_all_blocks_in_a_dag() {
        let storage_provider = MemoryStorage::default();
        let mut db = SphereDb::new(storage_provider).await.unwrap();

        let list1 = vec!["cats", "dogs", "pigeons"];
        let list2 = vec!["apples", "oranges", "starfruit"];

        let cid1 = db.save::<DagCborCodec, _>(&list1).await.unwrap();
        let cid2 = db.save::<DagCborCodec, _>(&list2).await.unwrap();

        let list3 = vec![cid1, cid2];

        let cid3 = db.save::<DagCborCodec, _>(&list3).await.unwrap();

        let stream = db.stream_blocks(&cid3);

        tokio::pin!(stream);

        let mut cids = Vec::new();

        while let Some((cid, block)) = stream.try_next().await.unwrap() {
            let derived_cid = derive_cid::<DagCborCodec>(&block);
            assert_eq!(cid, derived_cid);
            cids.push(cid);
        }

        assert_eq!(cids.len(), 3);

        for cid in [cid1, cid2, cid3] {
            assert!(cids.contains(&cid));
        }
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    pub async fn it_can_put_a_raw_block_and_read_it_as_a_token() {
        let storage_provider = MemoryStorage::default();
        let mut db = SphereDb::new(storage_provider).await.unwrap();

        let (cid, block) = block_encode::<RawCodec, _>(&Ipld::Bytes(b"foobar".to_vec())).unwrap();

        db.put_block(&cid, &block).await.unwrap();

        let token = db.read_token(&cid).await.unwrap();

        assert_eq!(token, Some("foobar".into()));
    }
}
