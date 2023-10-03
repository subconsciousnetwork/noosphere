use anyhow::{anyhow, Result};
use libipld_cbor::DagCborCodec;
use ucan::{builder::UcanBuilder, crypto::KeyMaterial};

use crate::{
    authority::{generate_capability, Authorization, SphereAbility},
    data::{
        ChangelogIpld, DelegationIpld, Did, IdentityIpld, Jwt, Link, MapOperation, MemoIpld,
        RevocationIpld, VersionedMapKey, VersionedMapValue,
    },
};

use noosphere_storage::{BlockStore, UcanStore};

#[cfg(doc)]
use crate::{
    data::VersionedMapIpld,
    view::versioned_map::{Content, Delegations, Identities, Revocations},
};

/// A [VersionedMapMutation] that corresponds to [Content]
pub type ContentMutation = VersionedMapMutation<String, Link<MemoIpld>>;

/// A [VersionedMapMutation] that corresponds to [Identities]
pub type IdentitiesMutation = VersionedMapMutation<String, IdentityIpld>;

/// A [VersionedMapMutation] that corresponds to [Delegations]
pub type DelegationsMutation = VersionedMapMutation<Link<Jwt>, DelegationIpld>;

/// A [VersionedMapMutation] that corresponds to [Revocations]
pub type RevocationsMutation = VersionedMapMutation<Link<Jwt>, RevocationIpld>;

#[cfg(doc)]
use crate::view::Sphere;

/// A [SphereRevision] represents a new, unsigned version of a [Sphere]. A
/// [SphereRevision] must be signed as a final step before the [Link<MemoIpld>]
/// of a new sphere version can be considered part of the official history of
/// the sphere. The credential used to sign must be authorized to create new
/// history by the sphere's key.
#[derive(Debug)]
pub struct SphereRevision<S: BlockStore> {
    /// The [Did] of the sphere that this revision corresponds to
    pub sphere_identity: Did,
    /// A [BlockStore] that contains blocks assocaited with the sphere that this
    /// revision corresponds to
    pub store: S,
    /// The unsigned memo that wraps the root of the [SphereIpld] for this
    /// sphere revision
    pub memo: MemoIpld,
}

impl<S: BlockStore> SphereRevision<S> {
    /// Sign the [SphereRevision] with the provided credential and return the
    /// [Link<MemoIpld>] pointing to the root of the new, signed revision
    pub async fn sign<Credential: KeyMaterial>(
        &mut self,
        credential: &Credential,
        authorization: Option<&Authorization>,
    ) -> Result<Link<MemoIpld>> {
        let proof = match authorization {
            Some(authorization) => {
                let witness_ucan = authorization
                    .as_ucan(&UcanStore(self.store.clone()))
                    .await?;

                Some(
                    UcanBuilder::default()
                        .issued_by(credential)
                        .for_audience(&self.sphere_identity)
                        .with_lifetime(120)
                        .witnessed_by(&witness_ucan, None)
                        .claiming_capability(&generate_capability(
                            &self.sphere_identity,
                            SphereAbility::Publish,
                        ))
                        .with_nonce()
                        .build()?
                        .sign()
                        .await?,
                )
            }
            None => None,
        };

        self.memo.sign(credential, proof.as_ref()).await?;
        Ok(self.store.save::<DagCborCodec, _>(&self.memo).await?.into())
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

impl SphereMutation {
    /// Initialize a new [SphereMutation] with the [Did] of the author of the
    /// mutation
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

    /// The [Did] of the author of the mutation
    pub fn did(&self) -> &str {
        &self.did
    }

    /// A mutable reference to the changes to sphere content, given as a
    /// [ContentMutation]
    pub fn content_mut(&mut self) -> &mut ContentMutation {
        &mut self.content
    }

    /// An immutable reference to the changes to sphere content, given as a
    /// [ContentMutation]
    pub fn content(&self) -> &ContentMutation {
        &self.content
    }

    /// A mutable reference to the changes to sphere identities (petnames),
    /// given as a [IdentitiesMutation]
    pub fn identities_mut(&mut self) -> &mut IdentitiesMutation {
        &mut self.identities
    }

    /// An immutable reference to the changes to sphere identities (petnames),
    /// given as a [IdentitiesMutation]
    pub fn identities(&self) -> &IdentitiesMutation {
        &self.identities
    }

    /// A mutable reference to the changes to sphere delegations (of authority),
    /// given as a [DelegationsMutation]
    pub fn delegations_mut(&mut self) -> &mut DelegationsMutation {
        &mut self.delegations
    }

    /// An immutable reference to the changes to sphere delegations (of
    /// authority), given as a [DelegationsMutation]
    pub fn delegations(&self) -> &DelegationsMutation {
        &self.delegations
    }

    /// A mutable reference to the changes to sphere revocations (of authority),
    /// given as a [RevocationsMutation]
    pub fn revocations_mut(&mut self) -> &mut RevocationsMutation {
        &mut self.revocations
    }

    /// An immutable reference to the changes to sphere revocations (of
    /// authority), given as a [RevocationsMutation]
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

    /// Consume a [SphereMutation], appending its changes to this one
    pub fn append(&mut self, other: SphereMutation) {
        self.content.changes = append_changes(
            std::mem::take(&mut self.content.changes),
            other.content.changes,
        );

        self.identities.changes = append_changes(
            std::mem::take(&mut self.identities.changes),
            other.identities.changes,
        );

        self.delegations.changes = append_changes(
            std::mem::take(&mut self.delegations.changes),
            other.delegations.changes,
        );

        self.revocations.changes = append_changes(
            std::mem::take(&mut self.revocations.changes),
            other.revocations.changes,
        );
    }
}

fn append_changes<K, V>(
    mut destination: Vec<MapOperation<K, V>>,
    source: Vec<MapOperation<K, V>>,
) -> Vec<MapOperation<K, V>>
where
    K: VersionedMapKey,
    V: VersionedMapValue,
{
    for change in source {
        let op_key = match &change {
            MapOperation::Add { key, .. } => key,
            MapOperation::Remove { key } => key,
        };

        destination.retain(|op| {
            let this_op_key = match &op {
                MapOperation::Add { key, .. } => key,
                MapOperation::Remove { key } => key,
            };

            this_op_key != op_key
        });

        destination.push(change);
    }

    destination
}

/// A generalized expression of a mutation to a [VersionedMapIpld]
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
    /// Set the changes as expressed by a [ChangelogIpld] to this
    /// [VersionedMapMutation]; the mutation will adopt the author of the
    /// changelog as its own author
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

    /// Initialize a new [VersionedMapMutation] whose author has the given [Did]
    pub fn new(did: &str) -> Self {
        VersionedMapMutation {
            did: did.into(),
            changes: Default::default(),
        }
    }

    /// Get the [Did] of the author of the [VersionedMapMutation]
    pub fn did(&self) -> &str {
        &self.did
    }

    /// Get the changes (as [MapOperation]s) represented by this
    /// [VersionedMapMutation]
    pub fn changes(&self) -> &[MapOperation<K, V>] {
        &self.changes
    }

    /// Record a change to the related [VersionedMapIpld] by key and value
    pub fn set(&mut self, key: &K, value: &V) {
        self.changes.push(MapOperation::Add {
            key: key.clone(),
            value: value.clone(),
        });
    }

    /// Remove a change from the [VersionedMapMutation]
    pub fn remove(&mut self, key: &K) {
        self.changes.push(MapOperation::Remove { key: key.clone() });
    }
}
