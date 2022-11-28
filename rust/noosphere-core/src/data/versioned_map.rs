use anyhow::Result;
use cid::Cid;
pub use crdts::{map, Orswot};
use libipld_cbor::DagCborCodec;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{fmt::Display, hash::Hash, marker::PhantomData};

use noosphere_collections::hamt::{Hamt, Hash as HamtHash, Sha256};
use noosphere_storage::BlockStore;

use super::ChangelogIpld;

#[cfg(not(target_arch = "wasm32"))]
pub trait VersionedMapSendSync: Send + Sync {}

#[cfg(not(target_arch = "wasm32"))]
impl<T> VersionedMapSendSync for T where T: Send + Sync {}

#[cfg(target_arch = "wasm32")]
pub trait VersionedMapSendSync {}

#[cfg(target_arch = "wasm32")]
impl<T> VersionedMapSendSync for T {}

#[repr(transparent)]
#[derive(Ord, Eq, PartialEq, PartialOrd, Debug, Clone, Serialize, Deserialize)]
pub struct CidKey(pub Cid);

impl HamtHash for CidKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash().hash(state);
    }
}

impl Display for CidKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

pub trait VersionedMapKey:
    Serialize + DeserializeOwned + HamtHash + Clone + Eq + Ord + VersionedMapSendSync + Display
{
}

impl<T> VersionedMapKey for T where
    T: Serialize + DeserializeOwned + HamtHash + Clone + Eq + Ord + VersionedMapSendSync + Display
{
}

pub trait VersionedMapValue:
    Serialize + DeserializeOwned + Clone + Eq + Hash + VersionedMapSendSync
{
}

impl<T> VersionedMapValue for T where
    T: Serialize + DeserializeOwned + Clone + Eq + Hash + VersionedMapSendSync
{
}

#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub enum MapOperation<Key, Value> {
    Add { key: Key, value: Value },
    Remove { key: Key },
}

#[derive(Debug, Default, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub struct VersionedMapIpld<Key, Value>
where
    Key: VersionedMapKey,
    Value: VersionedMapValue,
{
    /// A pointer to a HAMT
    pub hamt: Cid,
    // TODO: The size of this vec is implicitly limited by the IPLD block size
    // limit. This is probably fine most of the time; the vec only holds the
    // delta changes, and N<10 probably holds in the majority of cases. But, it
    // will be necessary to gracefully survive the outlier cases where N>~1000.
    // pub changelog: ChangelogIpld<MapOperation<Key, Value>>,
    pub changelog: Cid,

    #[serde(skip)]
    pub signature: PhantomData<(Key, Value)>,
}

impl<Key, Value> VersionedMapIpld<Key, Value>
where
    Key: VersionedMapKey,
    Value: VersionedMapValue,
{
    pub async fn try_load_hamt<S: BlockStore>(
        &self,
        store: &S,
    ) -> Result<Hamt<S, Value, Key, Sha256>> {
        Hamt::load(&self.hamt, store.clone()).await
    }

    pub async fn try_load_changelog<S: BlockStore>(
        &self,
        store: &S,
    ) -> Result<ChangelogIpld<MapOperation<Key, Value>>> {
        store.load::<DagCborCodec, _>(&self.changelog).await
    }

    // NOTE: We currently don't have a mechanism to prepuplate the store with
    // "empty" DAGs like a HAMT. So, we do it lazily by requiring async
    // initialization of this struct even when it is empty.
    pub async fn try_empty<S: BlockStore>(store: &mut S) -> Result<Self> {
        let mut hamt = Hamt::<S, Value, Key, Sha256>::new(store.clone());
        let changelog = ChangelogIpld::<MapOperation<Key, Value>>::default();

        let changelog_cid = store.save::<DagCborCodec, _>(&changelog).await?;
        let hamt_cid = hamt.flush().await?;

        Ok(VersionedMapIpld {
            hamt: hamt_cid,
            changelog: changelog_cid,
            signature: Default::default(),
        })
    }
}
