use anyhow::{anyhow, Result};
use cid::Cid;
use libipld_cbor::DagCborCodec;
use ucan::crypto::KeyMaterial;

use crate::{
    authority::Authorization,
    data::{
        ChangelogIpld, DelegationIpld, Did, IdentityIpld, Jwt, Link, MapOperation, MemoIpld,
        RevocationIpld, VersionedMapKey, VersionedMapValue,
    },
};

use noosphere_storage::BlockStore;

pub type ContentMutation = VersionedMapMutation<String, Link<MemoIpld>>;
pub type IdentitiesMutation = VersionedMapMutation<String, IdentityIpld>;
pub type DelegationsMutation = VersionedMapMutation<Link<Jwt>, DelegationIpld>;
pub type RevocationsMutation = VersionedMapMutation<Link<Jwt>, RevocationIpld>;

#[cfg(doc)]
use crate::view::Sphere;

/// A [SphereRevision] represents a new, unsigned version of a [Sphere]. A
/// [SphereRevision] must be signed as a final step before the [Cid] of a new
/// sphere version can be considered part of the official history of the sphere.
/// The credential used to sign must be authorized to create new history by the
/// sphere's key.
#[derive(Debug)]
pub struct SphereRevision<S: BlockStore> {
    pub store: S,
    pub memo: MemoIpld,
}

impl<S: BlockStore> SphereRevision<S> {
    pub async fn sign<Credential: KeyMaterial>(
        &mut self,
        credential: &Credential,
        authorization: Option<&Authorization>,
    ) -> Result<Cid> {
        self.memo.sign(credential, authorization).await?;
        self.store.save::<DagCborCodec, _>(&self.memo).await
    }
}

/// A [SphereMutation] is created and modified in order to describe changes to a
/// [Sphere]. After initializing the [SphereMutation], changes to the sphere are
/// made to it and then it is "applied" to the [Sphere] to produce a
/// [SphereRevision], which may then be signed.
#[derive(Debug)]
pub struct SphereMutation {
    did: Did,
    content: ContentMutation,
    identities: IdentitiesMutation,
    delegations: DelegationsMutation,
    revocations: RevocationsMutation,
}

impl<'a> SphereMutation {
    pub fn new(did: &str) -> Self {
        SphereMutation {
            did: did.into(),
            content: ContentMutation::new(did),
            identities: IdentitiesMutation::new(did),
            delegations: DelegationsMutation::new(did),
            revocations: RevocationsMutation::new(did),
        }
    }

    /// Get the identity of the author of this mutation
    pub fn author(&self) -> &Did {
        &self.did
    }

    /// Reset the state of the [SphereMutation], so that it may be re-used
    /// without being recreated. This is sometimes useful if the code that is
    /// working with the [SphereMutation] does not have sufficient information
    /// to set the author [Did] for a new [SphereMutation].
    pub fn reset(&mut self) {
        self.content = ContentMutation::new(&self.did);
        self.identities = IdentitiesMutation::new(&self.did);
        self.delegations = DelegationsMutation::new(&self.did);
        self.revocations = RevocationsMutation::new(&self.did);
    }

    pub fn did(&self) -> &str {
        &self.did
    }

    pub fn content_mut(&mut self) -> &mut ContentMutation {
        &mut self.content
    }

    pub fn content(&self) -> &ContentMutation {
        &self.content
    }

    pub fn identities_mut(&mut self) -> &mut IdentitiesMutation {
        &mut self.identities
    }

    pub fn identities(&self) -> &IdentitiesMutation {
        &self.identities
    }

    pub fn delegations_mut(&mut self) -> &mut DelegationsMutation {
        &mut self.delegations
    }

    pub fn delegations(&self) -> &DelegationsMutation {
        &self.delegations
    }

    pub fn revocations_mut(&mut self) -> &mut RevocationsMutation {
        &mut self.revocations
    }

    pub fn revocations(&self) -> &RevocationsMutation {
        &self.revocations
    }

    /// Returns true if no new changes would be made by applying this
    /// mutation to a [Sphere]. Otherwise, false.
    pub fn is_empty(&self) -> bool {
        self.content.changes.len() == 0
            && self.identities.changes.len() == 0
            && self.delegations.changes.len() == 0
            && self.revocations.changes.len() == 0
    }
}

#[derive(Default, Debug)]
pub struct VersionedMapMutation<K, V>
where
    K: VersionedMapKey,
    V: VersionedMapValue,
{
    did: String,
    changes: Vec<MapOperation<K, V>>,
}

impl<K, V> VersionedMapMutation<K, V>
where
    K: VersionedMapKey,
    V: VersionedMapValue,
{
    pub fn apply_changelog(&mut self, changelog: &ChangelogIpld<MapOperation<K, V>>) -> Result<()> {
        let did = changelog
            .did
            .as_ref()
            .ok_or_else(|| anyhow!("Changelog did not have an author DID"))?;

        if did != &self.did {
            return Err(anyhow!(
                "Changelog has unexpected author (was {}, expected {})",
                did,
                self.did
            ));
        }

        self.changes = changelog.changes.clone();

        Ok(())
    }

    pub fn new(did: &str) -> Self {
        VersionedMapMutation {
            did: did.into(),
            changes: Default::default(),
        }
    }
    pub fn did(&self) -> &str {
        &self.did
    }

    pub fn changes(&self) -> &[MapOperation<K, V>] {
        &self.changes
    }

    pub fn set(&mut self, key: &K, value: &V) {
        self.changes.push(MapOperation::Add {
            key: key.clone(),
            value: value.clone(),
        });
    }

    pub fn remove(&mut self, key: &K) {
        self.changes.push(MapOperation::Remove { key: key.clone() });
    }
}
