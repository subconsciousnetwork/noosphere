use anyhow::Result;
use cid::Cid;
pub use crdts::{map, Orswot};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{hash::Hash, marker::PhantomData};

use noosphere_cbor::TryDagCbor;
use noosphere_collections::hamt::{Hamt, Hash as HamtHash, Sha256};
use noosphere_storage::interface::{DagCborStore, Store};

use super::ChangelogIpld;

// pub type Changelog<Key, Value> = Vec<map::Op<Key, Orswot<Value, String>, String>>;

#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub enum MapOperation<Key, Value> {
    Add { key: Key, value: Value },
    Remove { key: Key },
}

#[derive(Debug, Default, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub struct VersionedMapIpld<Key, Value>
where
    Key: HamtHash + Eq + Ord,
    Value: Clone + Eq + Hash,
{
    /// A pointer to a HAMT
    pub hamt: Cid,
    // TODO: The size of this vec is implicitly limited by the IPLD block size
    // limit. This is probably fine most of the time; the vec only holds the
    // delta changes, and N<10 probably holds in the majority of cases. But, it
    // will be necessary to gracefully survive the outlier cases where N>~1000.
    // pub changelog: ChangelogIpld<MapOperation<Key, Value>>,
    pub changelog: Cid,

    pub signature: PhantomData<(Key, Value)>,
}

impl<Key, Value> VersionedMapIpld<Key, Value>
where
    Key: DeserializeOwned + Serialize + HamtHash + Eq + Ord,
    Value: DeserializeOwned + Serialize + Clone + Eq + Hash,
{
    pub async fn try_load_hamt<Storage: Store>(
        &self,
        store: &Storage,
    ) -> Result<Hamt<Storage, Value, Key, Sha256>> {
        println!("LOADING HAMT");
        Hamt::load(&self.hamt, store.clone()).await
    }

    pub async fn try_load_changelog<Storage: Store>(
        &self,
        store: &Storage,
    ) -> Result<ChangelogIpld<MapOperation<Key, Value>>> {
        store.load(&self.changelog).await
    }

    // NOTE: We currently don't have a mechanism to prepuplate the store with
    // "empty" DAGs like a HAMT. So, we do it lazily by requiring async
    // initialization of this struct even when it is empty.
    pub async fn try_empty<Storage: Store>(store: &mut Storage) -> Result<Self> {
        let mut hamt = Hamt::<Storage, Value, Key, Sha256>::new(store.clone());
        let changelog = ChangelogIpld::<MapOperation<Key, Value>>::default();

        let changelog_cid = store.write_cbor(&changelog.try_into_dag_cbor()?).await?;
        let hamt_cid = hamt.flush().await?;

        Ok(VersionedMapIpld {
            hamt: hamt_cid,
            changelog: changelog_cid,
            signature: Default::default(),
        })
    }
}
