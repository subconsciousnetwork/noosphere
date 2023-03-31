use std::{collections::BTreeMap, ops::Deref, pin::Pin};

use anyhow::{anyhow, Result};
use cid::Cid;
use futures::Stream;
use libipld_cbor::DagCborCodec;
use libipld_core::{
    codec::{Codec, Encode},
    ipld::Ipld,
};
use tokio::sync::OnceCell;

use crate::data::{
    AddressIpld, ChangelogIpld, CidKey, DelegationIpld, LinksIpld, MapOperation, MemoIpld,
    RevocationIpld, VersionedMapIpld, VersionedMapKey, VersionedMapValue,
};

use noosphere_collections::hamt::Hamt;
use noosphere_storage::{block_serialize, BlockStore};

use super::VersionedMapMutation;

pub type Links<S> = VersionedMap<String, MemoIpld, S>;
pub type Names<S> = VersionedMap<String, AddressIpld, S>;
pub type AllowedUcans<S> = VersionedMap<CidKey, DelegationIpld, S>;
pub type RevokedUcans<S> = VersionedMap<CidKey, RevocationIpld, S>;

/// A view over a [VersionedMapIpld] which provides high-level traversal of the
/// underlying data structure, including ergonomic access to its internal
/// [HAMT](https://ipld.io/specs/advanced-data-layouts/hamt/). The end-product is
/// a convenient view over key/value data in IPLD that includes versioning
/// information suitable to support multi-device synchronization over time.
#[derive(Debug)]
pub struct VersionedMap<K, V, S>
where
    K: VersionedMapKey,
    V: VersionedMapValue,
    S: BlockStore,
{
    cid: Cid,
    store: S,
    // NOTE: OnceCell used here for the caching benefits; it may not be necessary for changelog
    body: OnceCell<VersionedMapIpld<K, V>>,
    hamt: OnceCell<Hamt<S, V, K>>,
    changelog: OnceCell<ChangelogIpld<MapOperation<K, V>>>,
}

impl<K, V, S> VersionedMap<K, V, S>
where
    K: VersionedMapKey,
    V: VersionedMapValue,
    S: BlockStore,
{
    /// Loads the underlying IPLD (if it hasn't been loaded already) and returns
    /// an owned copy of it
    pub async fn to_body(&self) -> Result<VersionedMapIpld<K, V>> {
        Ok(self
            .body
            .get_or_try_init(|| async { self.store.load::<DagCborCodec, _>(&self.cid).await })
            .await?
            .clone())
    }

    pub async fn get_changelog(&self) -> Result<&ChangelogIpld<MapOperation<K, V>>> {
        self.changelog
            .get_or_try_init(|| async { self.load_changelog().await })
            .await
    }

    pub async fn load_changelog(&self) -> Result<ChangelogIpld<MapOperation<K, V>>> {
        let ipld = self.to_body().await?;
        self.store
            .load::<DagCborCodec, ChangelogIpld<MapOperation<K, V>>>(&ipld.changelog)
            .await
    }

    pub async fn get_hamt(&self) -> Result<&Hamt<S, V, K>> {
        self.hamt
            .get_or_try_init(|| async { self.load_hamt().await })
            .await
    }

    async fn load_hamt(&self) -> Result<Hamt<S, V, K>> {
        let ipld = self.to_body().await?;
        ipld.load_hamt(&self.store).await
    }

    pub async fn at_or_empty<C>(cid: Option<C>, store: &mut S) -> Result<VersionedMap<K, V, S>>
    where
        C: Deref<Target = Cid>,
    {
        Ok(match cid {
            Some(cid) => VersionedMap::<K, V, S>::at(&cid, store),
            None => VersionedMap::<K, V, S>::empty(store).await?,
        })
    }

    pub fn cid(&self) -> &Cid {
        &self.cid
    }

    pub fn at(cid: &Cid, store: &S) -> VersionedMap<K, V, S> {
        VersionedMap {
            cid: *cid,
            store: store.clone(),
            body: OnceCell::new(),
            hamt: OnceCell::new(),
            changelog: OnceCell::new(),
        }
    }

    pub async fn empty(store: &mut S) -> Result<VersionedMap<K, V, S>> {
        let ipld = VersionedMapIpld::<K, V>::empty(store).await?;
        let cid = store.save::<DagCborCodec, _>(ipld).await?;

        Ok(VersionedMap {
            cid,
            hamt: OnceCell::new(),
            body: OnceCell::new(),
            changelog: OnceCell::new(),
            store: store.clone(),
        })
    }

    /// Get a [BTreeMap] of all the added entries for this version of the
    /// [VersionedMap].
    pub async fn get_added(&self) -> Result<BTreeMap<K, V>> {
        let changelog = self.get_changelog().await?;
        let mut added = BTreeMap::new();
        for item in changelog.changes.iter() {
            match item {
                MapOperation::Add { key, value } => added.insert(key.clone(), value.clone()),
                MapOperation::Remove { .. } => continue,
            };
        }
        Ok(added)
    }

    /// Read a key from the map. You can think of this as analogous to reading
    /// a key from a hashmap, but note that this will load the underlying HAMT
    /// into memory if it has not yet been accessed.
    pub async fn get(&self, key: &K) -> Result<Option<&V>> {
        let hamt = self.get_hamt().await?;

        hamt.get(key).await
    }

    /// Get a [Cid] for a given [Codec] that refers to the value stored at the
    /// given key, if any.
    pub async fn get_as_cid<C>(&self, key: &K) -> Result<Option<Cid>>
    where
        C: Codec + Default,
        Ipld: Encode<C>,
        u64: From<C>,
    {
        // TODO: We should explore refering to values internally via links, so
        // that we can avoid this re-serialization just to derive a CID. This may
        // be incompatible with other IPLD HAMT implementations, but may be a
        // worthy optimization none-the-less.
        let hamt = self.get_hamt().await?;
        let value = hamt.get(key).await?;

        Ok(match value {
            Some(value) => Some(block_serialize::<C, _>(value)?.0),
            None => None,
        })
    }

    /// Same as `get`, but gives an error result if the key is not present in
    /// the underlying HAMT.
    pub async fn require(&self, key: &K) -> Result<&V> {
        self.get(key)
            .await?
            .ok_or_else(|| anyhow!("Key {} not found!", key))
    }

    /// Same as `get_as_cid`, but gives an error result if the key is not present in
    /// the underlying HAMT.
    pub async fn require_as_cid<C>(&self, key: &K) -> Result<Cid>
    where
        C: Codec + Default,
        Ipld: Encode<C>,
        u64: From<C>,
    {
        self.get_as_cid(key)
            .await?
            .ok_or_else(|| anyhow!("Key {} not found!", key))
    }

    pub async fn apply_with_cid<C>(
        cid: Option<C>,
        mutation: &VersionedMapMutation<K, V>,
        store: &mut S,
    ) -> Result<Cid>
    where
        C: Deref<Target = Cid>,
    {
        let map = Self::at_or_empty(cid, store).await?;
        let mut changelog = map.get_changelog().await?.mark(mutation.did());
        let mut hamt = map.load_hamt().await?;

        for change in mutation.changes() {
            match change {
                MapOperation::Add { key, value } => {
                    hamt.set(key.clone(), value.clone()).await?;
                }
                MapOperation::Remove { key } => {
                    hamt.delete(key).await?;
                }
            };

            changelog.push(change.clone())?;
        }

        let changelog_cid = store.save::<DagCborCodec, _>(&changelog).await?;
        let hamt_cid = hamt.flush().await?;
        let links_ipld = LinksIpld {
            hamt: hamt_cid,
            changelog: changelog_cid,
            ..Default::default()
        };

        store.save::<DagCborCodec, _>(&links_ipld).await
    }

    pub async fn for_each<ForEach>(&self, for_each: ForEach) -> Result<()>
    where
        ForEach: FnMut(&K, &V) -> Result<()>,
    {
        self.get_hamt().await?.for_each(for_each).await
    }

    pub async fn stream<'a>(
        &'a self,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<(&'a K, &'a V)>> + 'a>>> {
        Ok(self.get_hamt().await?.stream())
    }
}

impl<K, V, S> VersionedMap<K, V, S>
where
    K: VersionedMapKey + 'static,
    V: VersionedMapValue + 'static,
    S: BlockStore + 'static,
{
    pub async fn into_stream(self) -> Result<impl Stream<Item = Result<(K, V)>>> {
        Ok(self.load_hamt().await?.into_stream())
    }
}
