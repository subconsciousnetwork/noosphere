use std::pin::Pin;

use anyhow::Result;
use async_once_cell::OnceCell;
use cid::Cid;
use futures::Stream;
use libipld_cbor::DagCborCodec;

use crate::data::{
    AddressIpld, ChangelogIpld, CidKey, DelegationIpld, LinksIpld, MapOperation, RevocationIpld,
    VersionedMapIpld, VersionedMapKey, VersionedMapValue,
};

use noosphere_collections::hamt::Hamt;
use noosphere_storage::interface::BlockStore;

use super::VersionedMapMutation;

pub type Links<S> = VersionedMap<String, Cid, S>;
pub type Names<S> = VersionedMap<String, AddressIpld, S>;
pub type AllowedUcans<S> = VersionedMap<CidKey, DelegationIpld, S>;
pub type RevokedUcans<S> = VersionedMap<CidKey, RevocationIpld, S>;

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
    hamt: OnceCell<Hamt<S, V, K>>,
    changelog: OnceCell<ChangelogIpld<MapOperation<K, V>>>,
}

impl<K: VersionedMapKey, V: VersionedMapValue, S: BlockStore> VersionedMap<K, V, S> {
    pub async fn try_get_changelog(&self) -> Result<&ChangelogIpld<MapOperation<K, V>>> {
        self.changelog
            .get_or_try_init(async {
                let ipld = self
                    .store
                    .load::<DagCborCodec, VersionedMapIpld<K, V>>(&self.cid)
                    .await?;
                self.store
                    .load::<DagCborCodec, ChangelogIpld<MapOperation<K, V>>>(&ipld.changelog)
                    .await
            })
            .await
    }

    pub async fn try_get_hamt(&self) -> Result<&Hamt<S, V, K>> {
        self.hamt
            .get_or_try_init(async { self.try_load_hamt().await })
            .await
    }

    async fn try_load_hamt(&self) -> Result<Hamt<S, V, K>> {
        let ipld = self
            .store
            .load::<DagCborCodec, VersionedMapIpld<K, V>>(&self.cid)
            .await?;

        ipld.try_load_hamt(&self.store).await
    }

    pub async fn try_at_or_empty(
        cid: Option<&Cid>,
        store: &mut S,
    ) -> Result<VersionedMap<K, V, S>> {
        Ok(match cid {
            Some(cid) => VersionedMap::<K, V, S>::at(cid, store),
            None => VersionedMap::<K, V, S>::try_empty(store).await?,
        })
    }

    pub fn cid(&self) -> &Cid {
        &self.cid
    }

    pub fn at(cid: &Cid, store: &S) -> VersionedMap<K, V, S> {
        VersionedMap {
            cid: *cid,
            store: store.clone(),
            hamt: OnceCell::new(),
            changelog: OnceCell::new(),
        }
    }

    pub async fn try_empty(store: &mut S) -> Result<VersionedMap<K, V, S>> {
        let ipld = LinksIpld::try_empty(store).await?;
        let cid = store.save::<DagCborCodec, _>(ipld).await?;

        Ok(VersionedMap {
            cid,
            hamt: OnceCell::new(),
            changelog: OnceCell::new(),
            store: store.clone(),
        })
    }

    pub async fn get(&self, key: &K) -> Result<Option<&V>> {
        let hamt = self.try_get_hamt().await?;

        hamt.get(key).await
    }

    pub async fn try_apply_with_cid(
        cid: Option<&Cid>,
        mutation: &VersionedMapMutation<K, V>,
        store: &mut S,
    ) -> Result<Cid> {
        let map = Self::try_at_or_empty(cid, store).await?;
        let mut changelog = map.try_get_changelog().await?.mark(mutation.did());
        let mut hamt = map.try_load_hamt().await?;

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
        self.try_get_hamt().await?.for_each(for_each).await
    }

    pub async fn stream<'a>(
        &'a self,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<(&'a K, &'a V)>> + 'a>>> {
        Ok(self.try_get_hamt().await?.stream())
    }
}
