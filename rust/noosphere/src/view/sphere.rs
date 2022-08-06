use anyhow::{anyhow, Result};
use cid::Cid;
use ucan::{
    builder::UcanBuilder,
    capability::{Capability, Resource, With},
    chain::ProofChain,
    crypto::{did::DidParser, KeyMaterial},
    ucan::Ucan,
};

use crate::{
    authority::{
        ed25519_key_to_mnemonic, generate_ed25519_key, restore_ed25519_key, SphereAction,
        SphereReference, SPHERE_SEMANTICS,
    },
    data::{Bundle, ContentType, Header, MemoIpld, SphereIpld, TryBundle},
    view::{Links, SphereMutation, SphereRevision, Timeline},
};

use noosphere_storage::interface::{DagCborStore, Store};

pub const SPHERE_LIFETIME: u64 = 315360000000; // 10,000 years (arbitrarily high)

/// High-level Sphere I/O
pub struct Sphere<Storage: Store> {
    store: Storage,
    cid: Cid,
}

impl<Storage: Store> Sphere<Storage> {
    pub fn at(cid: &Cid, store: &Storage) -> Sphere<Storage> {
        Sphere {
            store: store.clone(),
            cid: cid.clone(),
        }
    }

    pub fn cid(&self) -> &Cid {
        &self.cid
    }

    pub async fn try_as_memo(&self) -> Result<MemoIpld> {
        self.store.load(&self.cid).await
    }

    pub async fn try_as_body(&self) -> Result<SphereIpld> {
        self.store.load(&self.try_as_memo().await?.body).await
    }

    pub async fn try_as_bundle(&self) -> Result<Bundle> {
        MemoIpld::try_bundle_with_cid(self.cid(), &self.store).await
    }

    pub async fn try_bundle_until_ancestor(&self, cid: Option<&Cid>) -> Result<Bundle> {
        Bundle::try_from_timeslice(
            &Timeline::new(&self.store).slice(&self.cid, cid),
            &self.store,
        )
        .await
    }

    pub async fn try_get_parent(&self) -> Result<Option<Sphere<Storage>>> {
        match self.try_as_memo().await?.parent {
            Some(cid) => Ok(Some(Sphere::at(&cid, &self.store))),
            None => Ok(None),
        }
    }

    pub async fn try_get_links(&self) -> Result<Links<Storage>> {
        let sphere = self.try_as_body().await?;

        Ok(Links::try_at_or_empty(sphere.links.as_ref(), &mut self.store.clone()).await?)
    }

    pub async fn try_get_identity(&self) -> Result<String> {
        let sphere = self.try_as_body().await?;

        Ok(sphere.identity)
    }

    pub async fn try_to_mutation(&self) -> Result<SphereMutation> {
        let memo: MemoIpld = self.store.load(&self.cid).await?;
        let author = memo
            .get_first_header(&Header::Author.to_string())
            .ok_or_else(|| anyhow!("No author header found"))?;
        let mut mutation = SphereMutation::new(&author);

        let links = self.try_get_links().await?;
        let changelog = links.try_get_changelog().await?;

        if !changelog.is_empty() {
            mutation.links_mut().try_apply_changelog(&changelog)?;
        }

        Ok(mutation)
    }

    pub async fn try_apply(&self, mutation: &SphereMutation) -> Result<SphereRevision<Storage>> {
        Sphere::try_apply_with_cid(self.cid(), mutation, &mut self.store.clone()).await
    }

    pub async fn try_apply_with_cid(
        cid: &Cid,
        mutation: &SphereMutation,
        store: &mut Storage,
    ) -> Result<SphereRevision<Storage>> {
        let links_mutation = mutation.links();
        let mut memo: MemoIpld = MemoIpld::branch_from(cid, store).await?;
        let mut sphere: SphereIpld = store.load(&memo.body).await?;

        sphere.links = match links_mutation.changes().len() > 0 {
            true => {
                Some(Links::try_apply_with_cid(sphere.links.as_ref(), links_mutation, store).await?)
            }
            false => sphere.links,
        };

        memo.body = store.save(&sphere).await?;

        Ok(SphereRevision {
            memo,
            store: store.clone(),
        })
    }

    pub async fn try_rebase(&self, onto: &Cid) -> Result<SphereRevision<Storage>> {
        Sphere::try_rebase_with_cid(self.cid(), onto, &mut self.store.clone()).await
    }

    pub async fn try_rebase_with_cid(
        cid: &Cid,
        onto: &Cid,
        store: &mut Storage,
    ) -> Result<SphereRevision<Storage>> {
        Sphere::try_apply_with_cid(
            onto,
            &Sphere::at(cid, store).try_to_mutation().await?,
            store,
        )
        .await
    }

    pub async fn try_hydrate(&self) -> Result<()> {
        debug!("HYDRATING REVISION {}", self.cid());
        Sphere::try_hydrate_with_cid(self.cid(), &mut self.store.clone()).await
    }

    pub async fn try_hydrate_with_cid(cid: &Cid, store: &mut Storage) -> Result<()> {
        let sphere = Sphere::at(cid, store);
        let memo = sphere.try_as_memo().await?;
        let base_cid = match memo.parent {
            Some(cid) => cid,
            None => {
                let mut base_sphere = SphereIpld::default();
                base_sphere.identity = sphere.try_get_identity().await?;
                let empty_dag = MemoIpld::for_body(store, &base_sphere).await?;
                store.save(&empty_dag).await?
            }
        };

        let hydrated_revision =
            Sphere::try_apply_with_cid(&base_cid, &sphere.try_to_mutation().await?, store).await?;

        if hydrated_revision.memo.body != memo.body {
            return Err(anyhow!(
                "Unexpected CID after hydration (expected: {}, actual: {})",
                memo.body,
                hydrated_revision.memo.body
            ));
        }

        Ok(())
    }

    pub async fn try_sync<Credential: KeyMaterial>(
        &self,
        old_base: &Cid,
        new_base: &Cid,
        credential: &Credential,
        proof: Option<&Ucan>,
    ) -> Result<Cid> {
        let mut store = self.store.clone();
        let timeline = Timeline::new(&self.store);
        let timeslice = timeline.slice(new_base, Some(old_base));
        let hydrate_cids = timeslice.try_to_chronological().await?;

        for (cid, _) in hydrate_cids.iter().skip(1) {
            Self::try_hydrate_with_cid(cid, &mut store).await?;
        }

        let timeslice = timeline.slice(self.cid(), Some(old_base));
        let rebase_revisions = timeslice.try_to_chronological().await?;

        let mut next_base = new_base.clone();

        for (cid, _) in rebase_revisions.iter().skip(1) {
            let mut revision = Sphere::try_rebase_with_cid(cid, &next_base, &mut store).await?;
            next_base = revision.try_sign(credential, proof).await?;
        }

        Ok(next_base)
    }

    pub async fn try_generate(
        owner_did: &str,
        store: &mut Storage,
    ) -> Result<(Sphere<Storage>, Ucan, String)> {
        let key_material = generate_ed25519_key();
        let mnemonic = ed25519_key_to_mnemonic(&key_material)?;
        let did = key_material.get_did().await?;
        let mut memo = MemoIpld::for_body(
            store,
            &SphereIpld {
                identity: did.clone(),
                links: None,
                sealed: None,
                revocations: None,
            },
        )
        .await?;

        memo.headers.push((
            Header::ContentType.to_string(),
            ContentType::Sphere.to_string(),
        ));

        let capability = Capability {
            with: With::Resource {
                kind: Resource::Scoped(SphereReference { did }),
            },
            can: SphereAction::Authorize,
        };

        let ucan = UcanBuilder::default()
            .issued_by(&key_material)
            .for_audience(owner_did)
            .with_lifetime(SPHERE_LIFETIME)
            .claiming_capability(&capability)
            .build()?
            .sign()
            .await?;

        memo.sign(&key_material, None).await?;

        let sphere_cid = store.save(&memo).await?;

        Ok((Sphere::at(&sphere_cid, &store), ucan, mnemonic))
    }

    pub async fn try_authorize(
        &self,
        mnemonic: &str,
        next_owner_did: &str,
        current_proof: &Ucan,
        did_parser: &mut DidParser,
    ) -> Result<Ucan> {
        let memo: MemoIpld = self.store.load(&self.cid).await?;
        let sphere: SphereIpld = self.store.load(&memo.body).await?;
        let did = sphere.identity;
        let restored_key = restore_ed25519_key(mnemonic)?;
        let restored_did = restored_key.get_did().await?;

        if did != restored_did {
            return Err(anyhow!("Incorrect mnemonic provided"));
        }

        let proof_chain = ProofChain::from_ucan(current_proof.clone(), did_parser).await?;

        let authorize_capability = Capability {
            with: With::Resource {
                kind: Resource::Scoped(SphereReference { did: did.clone() }),
            },
            can: SphereAction::Authorize,
        };

        let mut proof_is_valid = false;

        for info in proof_chain.reduce_capabilities(&SPHERE_SEMANTICS) {
            if info.capability.enables(&authorize_capability) && info.originators.contains(&did) {
                proof_is_valid = true;
                break;
            }
        }

        if !proof_is_valid {
            return Err(anyhow!(
                "Proof does not enable authorizing other identities"
            ));
        }

        // TODO(#21): Revoke old proof

        let ucan = UcanBuilder::default()
            .issued_by(&restored_key)
            .for_audience(next_owner_did)
            .with_lifetime(SPHERE_LIFETIME)
            .claiming_capability(&authorize_capability)
            .build()?
            .sign()
            .await?;

        Ok(ucan)
    }
}

#[cfg(test)]
mod tests {
    use cid::Cid;
    use futures::{pin_mut, StreamExt};
    use ucan::{
        builder::UcanBuilder,
        capability::{Capability, Resource, With},
        crypto::{did::DidParser, KeyMaterial},
        ucan::Ucan,
    };

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    use crate::{
        authority::{
            ed25519_key_to_mnemonic, generate_ed25519_key, SphereAction, SphereReference,
            SUPPORTED_KEYS,
        },
        data::Bundle,
        view::{Sphere, SphereMutation, Timeline, SPHERE_LIFETIME},
    };

    use noosphere_storage::{
        interface::{DagCborStore, Store},
        memory::MemoryStore,
    };

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_be_generated_and_later_restored() {
        let mut store = MemoryStore::default();

        let (sphere_cid, sphere_identity) = {
            let owner_key = generate_ed25519_key();
            let owner_did = owner_key.get_did().await.unwrap();
            let (sphere, _, _) = Sphere::try_generate(&owner_did, &mut store).await.unwrap();

            (
                sphere.cid().clone(),
                sphere.try_get_identity().await.unwrap(),
            )
        };

        let restored_sphere = Sphere::at(&sphere_cid, &store);
        let restored_identity = restored_sphere.try_get_identity().await.unwrap();

        assert_eq!(sphere_identity, restored_identity);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_may_authorize_a_different_key_after_being_created() {
        let mut store = MemoryStore::default();

        let (sphere, ucan, mnemonic) = {
            let owner_key = generate_ed25519_key();
            let owner_did = owner_key.get_did().await.unwrap();
            Sphere::try_generate(&owner_did, &mut store).await.unwrap()
        };

        let next_owner_key = generate_ed25519_key();
        let next_owner_did = next_owner_key.get_did().await.unwrap();

        let mut did_parser = DidParser::new(SUPPORTED_KEYS);
        let new_ucan = sphere
            .try_authorize(&mnemonic, &next_owner_did, &ucan, &mut did_parser)
            .await
            .unwrap();

        assert_ne!(ucan.audience(), new_ucan.audience());
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_wont_authorize_a_different_key_if_the_mnemonic_is_wrong() {
        let mut store = MemoryStore::default();

        let (sphere, ucan, _) = {
            let owner_key = generate_ed25519_key();
            let owner_did = owner_key.get_did().await.unwrap();
            Sphere::try_generate(&owner_did, &mut store).await.unwrap()
        };

        let next_owner_key = generate_ed25519_key();
        let next_owner_did = next_owner_key.get_did().await.unwrap();
        let incorrect_mnemonic = ed25519_key_to_mnemonic(&next_owner_key).unwrap();

        let mut did_parser = DidParser::new(SUPPORTED_KEYS);
        let authorize_result = sphere
            .try_authorize(&incorrect_mnemonic, &next_owner_did, &ucan, &mut did_parser)
            .await;

        assert!(authorize_result.is_err());
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_wont_authorize_a_different_key_if_the_proof_does_not_authorize_it() {
        let mut store = MemoryStore::default();

        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();
        let (sphere, ucan, mnemonic) = Sphere::try_generate(&owner_did, &mut store).await.unwrap();

        let next_owner_key = generate_ed25519_key();
        let next_owner_did = next_owner_key.get_did().await.unwrap();

        let insufficient_ucan = UcanBuilder::default()
            .issued_by(&owner_key)
            .for_audience(&next_owner_did)
            .with_lifetime(SPHERE_LIFETIME)
            .claiming_capability(&Capability {
                with: With::Resource {
                    kind: Resource::Scoped(SphereReference {
                        did: sphere.try_get_identity().await.unwrap(),
                    }),
                },
                can: SphereAction::Publish,
            })
            .witnessed_by(&ucan)
            .build()
            .unwrap()
            .sign()
            .await
            .unwrap();

        let mut did_parser = DidParser::new(SUPPORTED_KEYS);
        let authorize_result = sphere
            .try_authorize(
                &mnemonic,
                &next_owner_did,
                &insufficient_ucan,
                &mut did_parser,
            )
            .await;

        assert!(authorize_result.is_err());
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_assign_a_link_and_later_read_the_link() {
        let mut store = MemoryStore::default();
        let foo_cid = store.write_cbor(b"foo").await.unwrap();

        let sphere_cid = {
            let owner_key = generate_ed25519_key();
            let owner_did = owner_key.get_did().await.unwrap();
            let (sphere, ucan, _) = Sphere::try_generate(&owner_did, &mut store).await.unwrap();

            let mut mutation = SphereMutation::new(&owner_did);
            mutation.links_mut().set("foo", &foo_cid);

            let mut revision = sphere.try_apply(&mutation).await.unwrap();
            revision.try_sign(&owner_key, Some(&ucan)).await.unwrap()
        };

        let restored_sphere = Sphere::at(&sphere_cid, &store);
        let restored_links = restored_sphere.try_get_links().await.unwrap();
        let restored_foo_cid = restored_links.get("foo").await.unwrap().unwrap();

        assert_eq!(foo_cid, restored_foo_cid);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_creates_a_lineage_as_changes_are_saved() {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (mut sphere, ucan, _) = Sphere::try_generate(&owner_did, &mut store).await.unwrap();
        let mut lineage = vec![sphere.cid().clone()];

        for i in 0..2u8 {
            let mut mutation = SphereMutation::new(&owner_did);
            mutation
                .links_mut()
                .set("foo", &store.write_cbor(&[i]).await.unwrap());
            let mut revision = sphere.try_apply(&mutation).await.unwrap();
            let next_cid = revision.try_sign(&owner_key, Some(&ucan)).await.unwrap();

            sphere = Sphere::at(&next_cid, &store);
            lineage.push(next_cid);
        }

        assert_eq!(lineage.len(), 3);

        for cid in lineage.iter().rev() {
            assert_eq!(cid, sphere.cid());
            if let Some(parent) = sphere.try_get_parent().await.unwrap() {
                sphere = parent;
            }
        }
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_rebase_a_change_onto_a_parallel_lineage() {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (sphere, ucan, _) = Sphere::try_generate(&owner_did, &mut store).await.unwrap();

        let bar_cid = store.write_cbor(b"bar").await.unwrap();
        let baz_cid = store.write_cbor(b"baz").await.unwrap();
        let foobar_cid = store.write_cbor(b"foobar").await.unwrap();
        let flurb_cid = store.write_cbor(b"flurb").await.unwrap();

        let mut base_mutation = SphereMutation::new(&owner_did);
        base_mutation.links_mut().set("foo", &bar_cid);

        let mut base_revision = sphere.try_apply(&base_mutation).await.unwrap();

        let base_cid = base_revision
            .try_sign(&owner_key, Some(&ucan))
            .await
            .unwrap();

        let mut lineage_a_mutation = SphereMutation::new(&owner_did);
        lineage_a_mutation.links_mut().set("bar", &baz_cid);

        let mut lineage_a_revision =
            Sphere::try_apply_with_cid(&base_cid, &lineage_a_mutation, &mut store)
                .await
                .unwrap();
        let lineage_a_cid = lineage_a_revision
            .try_sign(&owner_key, Some(&ucan))
            .await
            .unwrap();

        let mut lineage_b_mutation = SphereMutation::new(&owner_did);
        lineage_b_mutation.links_mut().set("foo", &foobar_cid);
        lineage_b_mutation.links_mut().set("baz", &flurb_cid);

        let mut lineage_b_revision =
            Sphere::try_apply_with_cid(&base_cid, &lineage_b_mutation, &mut store)
                .await
                .unwrap();
        let lineage_b_cid = lineage_b_revision
            .try_sign(&owner_key, Some(&ucan))
            .await
            .unwrap();

        let mut rebase_revision =
            Sphere::try_rebase_with_cid(&lineage_b_cid, &lineage_a_cid, &mut store)
                .await
                .unwrap();
        let rebase_cid = rebase_revision
            .try_sign(&owner_key, Some(&ucan))
            .await
            .unwrap();

        let rebased_sphere = Sphere::at(&rebase_cid, &store);
        let rebased_links = rebased_sphere.try_get_links().await.unwrap();

        let parent_sphere = rebased_sphere.try_get_parent().await.unwrap().unwrap();
        assert_eq!(parent_sphere.cid(), &lineage_a_cid);

        let grandparent_sphere = parent_sphere.try_get_parent().await.unwrap().unwrap();
        assert_eq!(grandparent_sphere.cid(), &base_cid);

        assert_eq!(rebased_links.get("foo").await.unwrap(), Some(foobar_cid));
        assert_eq!(rebased_links.get("bar").await.unwrap(), Some(baz_cid));
        assert_eq!(rebased_links.get("baz").await.unwrap(), Some(flurb_cid));
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_hydrate_revisions_from_sparse_link_blocks() {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (mut sphere, ucan, _) = Sphere::try_generate(&owner_did, &mut store).await.unwrap();

        for i in 0..32u8 {
            let mut mutation = SphereMutation::new(&owner_did);
            let key = format!("key{}", i);

            mutation
                .links_mut()
                .set(&key, &store.write_cbor(&[i]).await.unwrap());
            let mut revision = sphere.try_apply(&mutation).await.unwrap();
            let next_cid = revision.try_sign(&owner_key, Some(&ucan)).await.unwrap();
            sphere = Sphere::at(&next_cid, &store);
        }

        let bundle = sphere.try_bundle_until_ancestor(None).await.unwrap();
        let mut other_store = MemoryStore::default();

        for (_, block) in bundle.map() {
            other_store.write_cbor(block).await.unwrap();
        }

        let timeline = Timeline::new(&other_store);
        let timeslice = timeline.slice(sphere.cid(), None);
        let items = timeslice.try_to_chronological().await.unwrap();

        for (cid, _) in items {
            Sphere::at(&cid, &other_store).try_hydrate().await.unwrap();
        }

        store.expect_replica_in(&other_store).await.unwrap();
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_sync_a_lineage_with_external_changes() {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        async fn make_revision<Credential: KeyMaterial, Storage: Store>(
            base_cid: &Cid,
            author_did: &str,
            credential: &Credential,
            proof: &Ucan,
            store: &mut Storage,
            (change_key, change_cid): (&str, &Cid),
        ) -> anyhow::Result<Cid> {
            let mut mutation = SphereMutation::new(author_did);
            mutation.links_mut().set(change_key, change_cid);

            let mut base_revision = Sphere::try_apply_with_cid(base_cid, &mutation, store).await?;

            Ok(base_revision.try_sign(credential, Some(&proof)).await?)
        }

        let foo_cid = store.write_cbor(b"foo").await.unwrap();
        let bar_cid = store.write_cbor(b"bar").await.unwrap();
        let baz_cid = store.write_cbor(b"baz").await.unwrap();
        let foobar_cid = store.write_cbor(b"foobar").await.unwrap();
        let flurb_cid = store.write_cbor(b"flurb").await.unwrap();

        let (sphere, ucan, _) = Sphere::try_generate(&owner_did, &mut store).await.unwrap();

        let base_cid = make_revision(
            sphere.cid(),
            &owner_did,
            &owner_key,
            &ucan,
            &mut store,
            ("foo", &foo_cid),
        )
        .await
        .unwrap();

        let mut external_store = store.fork().await;

        let external_cid_a = make_revision(
            &base_cid,
            &owner_did,
            &owner_key,
            &ucan,
            &mut external_store,
            ("bar", &bar_cid),
        )
        .await
        .unwrap();

        let external_cid_b = make_revision(
            &external_cid_a,
            &owner_did,
            &owner_key,
            &ucan,
            &mut external_store,
            ("foobar", &foobar_cid),
        )
        .await
        .unwrap();

        let external_bundle = Bundle::try_from_timeslice(
            &Timeline::new(&external_store).slice(&external_cid_b, Some(&external_cid_a)),
            &external_store,
        )
        .await
        .unwrap();

        let local_cid_a = make_revision(
            &base_cid,
            &owner_did,
            &owner_key,
            &ucan,
            &mut store,
            ("baz", &baz_cid),
        )
        .await
        .unwrap();

        let local_cid_b = make_revision(
            &local_cid_a,
            &owner_did,
            &owner_key,
            &ucan,
            &mut store,
            ("bar", &flurb_cid),
        )
        .await
        .unwrap();

        external_bundle.load_into(&mut store).await.unwrap();

        let local_sphere = Sphere::at(&local_cid_b, &store);

        let _final_cid = local_sphere
            .try_sync(&base_cid, &external_cid_b, &owner_key, Some(&ucan))
            .await
            .unwrap();
    }
}
