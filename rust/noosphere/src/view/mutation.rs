use anyhow::{anyhow, Result};
use cid::Cid;
use libipld_cbor::DagCborCodec;
use ucan::crypto::KeyMaterial;

use crate::{
    authority::Authorization,
    data::{
        ChangelogIpld, CidKey, DelegationIpld, MapOperation, MemoIpld, RevocationIpld,
        VersionedMapKey, VersionedMapValue,
    },
};

use noosphere_storage::interface::BlockStore;

#[derive(Debug)]
pub struct SphereRevision<S: BlockStore> {
    pub store: S,
    pub memo: MemoIpld,
}

impl<S: BlockStore> SphereRevision<S> {
    pub async fn try_sign<Credential: KeyMaterial>(
        &mut self,
        credential: &Credential,
        authorization: Option<&Authorization>,
    ) -> Result<Cid> {
        self.memo.sign(credential, authorization).await?;
        self.store.save::<DagCborCodec, _>(&self.memo).await
    }
}

#[derive(Debug)]
pub struct SphereMutation {
    did: String,
    links: LinksMutation,
    allowed_ucans: AllowedUcansMutation,
    revoked_ucans: RevokedUcansMutation,
}

impl<'a> SphereMutation {
    pub fn new(did: &str) -> Self {
        SphereMutation {
            did: did.into(),
            links: LinksMutation::new(did),
            allowed_ucans: AllowedUcansMutation::new(did),
            revoked_ucans: RevokedUcansMutation::new(did),
        }
    }

    pub fn did(&self) -> &str {
        &self.did
    }

    pub fn links_mut(&mut self) -> &mut LinksMutation {
        &mut self.links
    }

    pub fn links(&self) -> &LinksMutation {
        &self.links
    }

    pub fn allowed_ucans_mut(&mut self) -> &mut AllowedUcansMutation {
        &mut self.allowed_ucans
    }

    pub fn allowed_ucans(&self) -> &AllowedUcansMutation {
        &self.allowed_ucans
    }

    pub fn revoked_ucans_mut(&mut self) -> &mut RevokedUcansMutation {
        &mut self.revoked_ucans
    }

    pub fn revoked_ucans(&self) -> &RevokedUcansMutation {
        &self.revoked_ucans
    }

    pub fn is_empty(&self) -> bool {
        self.links.changes.len() == 0
            && self.allowed_ucans.changes.len() == 0
            && self.revoked_ucans.changes.len() == 0
    }
}

pub type LinksMutation = VersionedMapMutation<String, Cid>;
pub type AllowedUcansMutation = VersionedMapMutation<CidKey, DelegationIpld>;
pub type RevokedUcansMutation = VersionedMapMutation<CidKey, RevocationIpld>;

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
    pub fn try_apply_changelog(
        &mut self,
        changelog: &ChangelogIpld<MapOperation<K, V>>,
    ) -> Result<()> {
        let did = changelog
            .did
            .as_ref()
            .ok_or_else(|| anyhow!("Changelog did not have an author DID"))?;

        // println!("CHANGELOG INSIDE MUTATION: {:?}", changelog);

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
