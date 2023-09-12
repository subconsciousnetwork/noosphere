use std::{collections::BTreeMap, marker::PhantomData, ops::Deref, pin::Pin};

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
    ChangelogIpld, ContentIpld, DelegationIpld, IdentityIpld, Jwt, Link, MapOperation, MemoIpld,
    RevocationIpld, VersionedMapIpld, VersionedMapKey, VersionedMapValue,
};

use noosphere_collections::hamt::Hamt;
use noosphere_storage::{block_serialize, BlockStore};

use super::VersionedMapMutation;

/// A [VersionedMap] that represents the content space of a sphere
pub type Content<S> = VersionedMap<String, Link<MemoIpld>, S>;
/// A [VersionedMap] that represents the petname space of a sphere
pub type Identities<S> = VersionedMap<String, IdentityIpld, S>;
/// A [VersionedMap] that represents the key authorizations in a sphere
pub type Delegations<S> = VersionedMap<Link<Jwt>, DelegationIpld, S>;
/// A [VersionedMap] that represents the authority revocations in a sphere
pub type Revocations<S> = VersionedMap<Link<Jwt>, RevocationIpld, S>;

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

    /// Get the [ChangelogIpld] for the underlying [VersionedMapIpld], loading
    /// it from storage if it is not available
    pub async fn get_changelog(&self) -> Result<&ChangelogIpld<MapOperation<K, V>>> {
        self.changelog
            .get_or_try_init(|| async { self.load_changelog().await })
            .await
    }

    /// Load the [ChangelogIpld] for the underlying [VersionedMapIpld] from
    /// storage
    pub async fn load_changelog(&self) -> Result<ChangelogIpld<MapOperation<K, V>>> {
        let ipld = self.to_body().await?;
        self.store
            .load::<DagCborCodec, ChangelogIpld<MapOperation<K, V>>>(&ipld.changelog)
            .await
    }

    /// Get the [Hamt] for the underlying [VersionedMapIpld], loading it from
    /// storage if it is not available
    pub async fn get_hamt(&self) -> Result<&Hamt<S, V, K>> {
        self.hamt
            .get_or_try_init(|| async { self.load_hamt().await })
            .await
    }

    /// Load the [Hamt] for the underlying [VersionedMapIpld] from storage
    async fn load_hamt(&self) -> Result<Hamt<S, V, K>> {
        let ipld = self.to_body().await?;
        ipld.load_hamt(&self.store).await
    }

    /// Initialize the [VersionedMap] over a [VersionedMapIpld] referred to by its [Cid] if known, or else
    /// a newly-initialized, empty [VersionedMapIpld].
    pub async fn at_or_empty<C>(cid: Option<C>, store: &mut S) -> Result<VersionedMap<K, V, S>>
    where
        C: Deref<Target = Cid>,
    {
        Ok(match cid {
            Some(cid) => VersionedMap::<K, V, S>::at(&cid, store),
            None => VersionedMap::<K, V, S>::empty(store).await?,
        })
    }

    /// Get the [Cid] of the underlying [VersionedMapIpld]
    pub fn cid(&self) -> &Cid {
        &self.cid
    }

    /// Initialize the [VersionedMap] over a [VersionedMapIpld] referred to by its [Cid]
    pub fn at(cid: &Cid, store: &S) -> VersionedMap<K, V, S> {
        VersionedMap {
            cid: *cid,
            store: store.clone(),
            body: OnceCell::new(),
            hamt: OnceCell::new(),
            changelog: OnceCell::new(),
        }
    }

    /// Initialize and store an empty [VersionedMapIpld], configuring a [VersionedMap] to
    /// point to it by its [Cid]
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

    /// Apply a [VersionedMapMutation] to the underlying [VersionedMapIpld] by iterating
    /// over the changes in the mutation and performing them one at a time.
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
        let links_ipld = ContentIpld {
            hamt: hamt_cid,
            changelog: changelog_cid,
            signature: PhantomData,
        };

        store.save::<DagCborCodec, _>(&links_ipld).await
    }

    /// Iterate over the keys and values of the underlying [VersionedMapIpld]
    /// sequentially. Note: consider using [VersionedMap::stream] instead for
    /// better ergonomics.
    pub async fn for_each<ForEach>(&self, for_each: ForEach) -> Result<()>
    where
        ForEach: FnMut(&K, &V) -> Result<()>,
    {
        self.get_hamt().await?.for_each(for_each).await
    }

    /// Produce a [Stream] that yields `(key, value)` tuples for all entries
    /// in the underlying [VersionedMapIpld].
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
    /// Consume the [VersionedMap] and produce a [Stream] that yields `(key,
    /// value)` tuples for all entries in the underlying [VersionedMapIpld].
    pub async fn into_stream(self) -> Result<impl Stream<Item = Result<(K, V)>>> {
        Ok(self.load_hamt().await?.into_stream())
    }
}
