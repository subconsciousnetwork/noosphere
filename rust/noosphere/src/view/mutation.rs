use anyhow::{anyhow, Result};
use cid::Cid;
use ucan::{crypto::KeyMaterial, ucan::Ucan};

use crate::data::{LinksChangelogIpld, LinksOperation, MemoIpld};

use noosphere_storage::interface::{DagCborStore, Store};

#[derive(Debug)]
pub struct SphereRevision<Storage: Store> {
    pub store: Storage,
    pub memo: MemoIpld,
}

impl<Storage: Store> SphereRevision<Storage> {
    // TODO: It would be nice if this was internally mutable
    pub async fn try_sign<Credential: KeyMaterial>(
        &mut self,
        credential: &Credential,
        proof: Option<&Ucan>,
    ) -> Result<Cid> {
        self.memo.sign(credential, proof).await?;
        self.store.save(&self.memo).await
    }
}

#[derive(Debug)]
pub struct SphereMutation {
    did: String,
    links: LinksMutation,
}

impl<'a> SphereMutation {
    pub fn new(did: &str) -> Self {
        SphereMutation {
            did: did.into(),
            links: LinksMutation::new(did),
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
}

#[derive(Default, Debug)]
pub struct LinksMutation {
    did: String,
    changes: Vec<LinksOperation>,
}

impl LinksMutation {
    pub fn try_apply_changelog(&mut self, changelog: &LinksChangelogIpld) -> Result<()> {
        let did = changelog
            .did
            .as_ref()
            .ok_or_else(|| anyhow!("Changelog did not have an author DID"))?;

        if did != &self.did {
            return Err(anyhow!("Changelog has unexpected author"));
        }

        self.changes = changelog.changes.clone();

        Ok(())
    }

    pub fn new(did: &str) -> Self {
        LinksMutation {
            did: did.into(),
            changes: Default::default(),
        }
    }
    pub fn did(&self) -> &str {
        &self.did
    }

    pub fn changes(&self) -> &[LinksOperation] {
        &self.changes
    }

    pub fn set(&mut self, slug: &str, value: &Cid) {
        self.changes.push(LinksOperation::Add {
            key: slug.into(),
            value: value.clone(),
        });
    }

    pub fn remove(&mut self, slug: &str) {
        self.changes
            .push(LinksOperation::Remove { key: slug.into() });
    }
}
