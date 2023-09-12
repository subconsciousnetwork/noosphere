use anyhow::Result;
use cid::Cid;
use libipld_cbor::DagCborCodec;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{fmt::Display, hash::Hash, marker::PhantomData};

use noosphere_collections::hamt::{Hamt, Hash as HamtHash, Sha256};
use noosphere_storage::BlockStore;

use noosphere_common::ConditionalSync;

use super::{ChangelogIpld, DelegationIpld, IdentityIpld, Jwt, Link, MemoIpld, RevocationIpld};

/// A [VersionedMapIpld] that represents the content space of a sphere
pub type ContentIpld = VersionedMapIpld<String, Link<MemoIpld>>;
/// A [VersionedMapIpld] that represents the petname space of a sphere
pub type IdentitiesIpld = VersionedMapIpld<String, IdentityIpld>;
/// A [VersionedMapIpld] that represents the key authorizations in a sphere
pub type DelegationsIpld = VersionedMapIpld<Link<Jwt>, DelegationIpld>;
/// A [VersionedMapIpld] that represents the authority revocations in a sphere
pub type RevocationsIpld = VersionedMapIpld<Link<Jwt>, RevocationIpld>;

/// A helper trait to simplify expressing the bounds of a valid [VersionedMapIpld] key
pub trait VersionedMapKey:
    Serialize + DeserializeOwned + HamtHash + Clone + Eq + Ord + ConditionalSync + Display
{
}

impl<T> VersionedMapKey for T where
    T: Serialize + DeserializeOwned + HamtHash + Clone + Eq + Ord + ConditionalSync + Display
{
}

/// A helper trait to simplify expressing the bounds of a valid [VersionedMapIpld] value
pub trait VersionedMapValue:
    Serialize + DeserializeOwned + Clone + Eq + Hash + ConditionalSync
{
}

impl<T> VersionedMapValue for T where
    T: Serialize + DeserializeOwned + Clone + Eq + Hash + ConditionalSync
{
}

/// A [MapOperation] represents a single change to a [VersionedMapIpld] as it may
/// be recorded in a [ChangelogIpld].
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub enum MapOperation<Key, Value> {
    /// A [MapOperation] that represents an update or insert to a [VersionedMapIpld]
    Add {
        /// The key that was updated or inserted
        key: Key,
        /// The new value associated with the key
        value: Value,
    },
    /// A [MapOperation] that represents a removal of a key from a [VersionedMapIpld]
    Remove {
        /// The key that was removed
        key: Key,
    },
}

/// A [VersionedMapIpld] pairs a [Hamt] and a [ChangelogIpld] to enable a data structure
/// that contains its difference from a historical ancestor without requiring that a diff
/// be performed.
#[derive(Debug, Default, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub struct VersionedMapIpld<Key, Value>
where
    Key: VersionedMapKey,
    Value: VersionedMapValue,
{
    /// A pointer to a [Hamt] root
    pub hamt: Cid,
    // TODO(#262): The size of this vec is implicitly limited by the IPLD block
    // size limit. This is probably fine most of the time; the vec only holds
    // the delta changes, and N<10 probably holds in the majority of cases. But,
    // it will be necessary to gracefully survive the outlier cases where
    // N>~1000.
    /// A pointer to a [ChangelogIpld]
    pub changelog: Cid,

    #[allow(missing_docs)]
    #[serde(skip)]
    pub signature: PhantomData<(Key, Value)>,
}

impl<Key, Value> VersionedMapIpld<Key, Value>
where
    Key: VersionedMapKey,
    Value: VersionedMapValue,
{
    /// Load the [Hamt] root associated with this [VersionedMapIpld]
    pub async fn load_hamt<S: BlockStore>(&self, store: &S) -> Result<Hamt<S, Value, Key, Sha256>> {
        Hamt::load(&self.hamt, store.clone()).await
    }

    /// Load the [ChangelogIpld] associated with this [VersionedMapIpld]
    pub async fn load_changelog<S: BlockStore>(
        &self,
        store: &S,
    ) -> Result<ChangelogIpld<MapOperation<Key, Value>>> {
        store.load::<DagCborCodec, _>(&self.changelog).await
    }

    /// Initialize an empty [VersionedMapIpld], creating an empty [Hamt] root
    /// and [ChangelogIpld] as well
    // NOTE: We currently don't have a mechanism to prepuplate the store with
    // "empty" DAGs like a HAMT. So, we do it lazily by requiring async
    // initialization of this struct even when it is empty.
    pub async fn empty<S: BlockStore>(store: &mut S) -> Result<Self> {
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
