use anyhow::{anyhow, Result};
use cid::Cid;
use libipld_cbor::DagCborCodec;

use ucan::{
    builder::UcanBuilder,
    capability::{Capability, Resource, With},
    chain::ProofChain,
    crypto::{did::DidParser, KeyMaterial},
    store::UcanJwtStore,
};

use crate::{
    authority::{
        ed25519_key_to_mnemonic, generate_ed25519_key, restore_ed25519_key, Authorization,
        SphereAction, SphereReference, SPHERE_SEMANTICS,
    },
    data::{
        AuthorityIpld, Bundle, CidKey, ContentType, DelegationIpld, Did, Header, MemoIpld,
        RevocationIpld, SphereIpld, TryBundle, Version,
    },
    view::{Links, SphereMutation, SphereRevision, Timeline},
};

use noosphere_storage::{interface::BlockStore, ucan::UcanStore};

use super::{AllowedUcans, Authority, Names, RevokedUcans};

pub const SPHERE_LIFETIME: u64 = 315360000000; // 10,000 years (arbitrarily high)

/// High-level Sphere I/O
pub struct Sphere<S: BlockStore> {
    store: S,
    cid: Cid,
}

impl<S: BlockStore> Sphere<S> {
    pub fn at(cid: &Cid, store: &S) -> Sphere<S> {
        Sphere {
            store: store.clone(),
            cid: *cid,
        }
    }

    /// Get the CID that points to the sphere's wrapping memo that corresponds
    /// to this revision of the sphere
    pub fn cid(&self) -> &Cid {
        &self.cid
    }

    /// Load the wrapping [MemoIpld] of the sphere data
    pub async fn try_as_memo(&self) -> Result<MemoIpld> {
        self.store.load::<DagCborCodec, _>(&self.cid).await
    }

    /// Load the body [SphereIpld] of this sphere
    pub async fn try_as_body(&self) -> Result<SphereIpld> {
        self.store
            .load::<DagCborCodec, _>(&self.try_as_memo().await?.body)
            .await
    }

    /// Produce a bundle that contains the sparse set of blocks needed
    /// to produce this revision to the sphere
    pub async fn try_as_bundle(&self) -> Result<Bundle> {
        MemoIpld::try_bundle_with_cid(self.cid(), &self.store).await
    }

    /// Produce a bundle that contains the sparse set of blocks needed to
    /// produce a series of sequential revisions of this sphere, up to but
    /// excluding the given [Cid] (or until the genesis revision of the
    /// sphere if no [Cid] is given).
    pub async fn try_bundle_until_ancestor(&self, cid: Option<&Cid>) -> Result<Bundle> {
        Bundle::try_from_timeslice(
            &Timeline::new(&self.store).slice(&self.cid, cid),
            &self.store,
        )
        .await
    }

    /// Get a [Sphere] view over the parent revision of the sphere relative to
    /// this revision, if one exists
    pub async fn try_get_parent(&self) -> Result<Option<Sphere<S>>> {
        match self.try_as_memo().await?.parent {
            Some(cid) => Ok(Some(Sphere::at(&cid, &self.store))),
            None => Ok(None),
        }
    }

    /// Attempt to load the [Links] of this sphere. If no links have been
    /// set for this sphere yet, this initializes an empty [Links] and returns
    /// it for the caller to populate.
    pub async fn try_get_links(&self) -> Result<Links<S>> {
        let sphere = self.try_as_body().await?;

        Links::try_at_or_empty(sphere.links.as_ref(), &mut self.store.clone()).await
    }

    /// Attempt to load the [Authority] of this sphere. If no authorizations or
    /// revocations have been added to this sphere yet, this initializes an
    /// empty [Authority] and returns it for the caller to populate.
    pub async fn try_get_authority(&self) -> Result<Authority<S>> {
        let sphere = self.try_as_body().await?;

        Authority::try_at_or_empty(sphere.authorization.as_ref(), &mut self.store.clone()).await
    }

    /// Attempt to load the [Names] of this sphere. If no names have been added
    /// to this sphere yet, this initializes an empty [Names] and returns it
    /// for the caller to populate.
    pub async fn try_get_names(&self) -> Result<Names<S>> {
        let sphere = self.try_as_body().await?;

        Names::try_at_or_empty(sphere.names.as_ref(), &mut self.store.clone()).await
    }

    /// Get the [Did] identity of the sphere
    pub async fn try_get_identity(&self) -> Result<Did> {
        let sphere = self.try_as_body().await?;

        Ok(sphere.identity)
    }

    /// Derive the mutation that would be required to produce the current
    /// sphere revision given its immediate ancestral parent. Note that
    /// this only considers changes that are internal to the sphere, and is
    /// not inclusive of the headers of the sphere's memo.
    pub async fn try_derive_mutation(&self) -> Result<SphereMutation> {
        let memo = self.try_as_memo().await?;
        let author = memo
            .get_first_header(&Header::Author.to_string())
            .ok_or_else(|| anyhow!("No author header found"))?;

        let mut mutation = SphereMutation::new(&author);

        let parent = match self.try_get_parent().await? {
            Some(parent) => parent,
            None => return Ok(mutation),
        };

        let parent_links = parent.try_get_links().await?;
        let links = self.try_get_links().await?;

        if links.cid() != parent_links.cid() {
            let changelog = links.try_get_changelog().await?;

            if changelog.is_empty() {
                return Err(anyhow!("Links have changed but the changelog is empty"));
            }

            mutation.links_mut().try_apply_changelog(changelog)?;
        }

        let parent_names = parent.try_get_names().await?;
        let names = self.try_get_names().await?;

        if names.cid() != parent_names.cid() {
            let changelog = names.try_get_changelog().await?;

            if changelog.is_empty() {
                return Err(anyhow!("Names have changed but the changelog is empty"));
            }

            mutation.names_mut().try_apply_changelog(changelog)?;
        }

        let parent_authorization = parent.try_get_authority().await?;
        let authorization = self.try_get_authority().await?;

        if authorization.cid() != parent_authorization.cid() {
            let parent_allowed_ucans = parent_authorization.try_get_allowed_ucans().await?;
            let allowed_ucans = authorization.try_get_allowed_ucans().await?;

            if allowed_ucans.cid() != parent_allowed_ucans.cid() {
                let changelog = allowed_ucans.try_get_changelog().await?;

                if changelog.is_empty() {
                    return Err(anyhow!("Allowed UCANs changed but the changelog is empty"));
                }

                mutation
                    .allowed_ucans_mut()
                    .try_apply_changelog(changelog)?;
            }

            let parent_revoked_ucans = parent_authorization.try_get_revoked_ucans().await?;
            let revoked_ucans = authorization.try_get_revoked_ucans().await?;

            if revoked_ucans.cid() != parent_revoked_ucans.cid() {
                let changelog = revoked_ucans.try_get_changelog().await?;

                if changelog.is_empty() {
                    return Err(anyhow!("Revoked UCANs changed but the changelog is empty"));
                }

                mutation
                    .revoked_ucans_mut()
                    .try_apply_changelog(changelog)?;
            }
        }

        Ok(mutation)
    }

    /// Apply a mutation to the sphere, producing a new sphere revision that
    /// must then be signed as an additional step.
    pub async fn try_apply_mutation(&self, mutation: &SphereMutation) -> Result<SphereRevision<S>> {
        Sphere::try_apply_mutation_with_cid(self.cid(), mutation, &mut self.store.clone()).await
    }

    /// Apply a mutation to the sphere given a revision CID, producing a new
    /// sphere revision that must then be signed as an additional step
    pub async fn try_apply_mutation_with_cid(
        cid: &Cid,
        mutation: &SphereMutation,
        store: &mut S,
    ) -> Result<SphereRevision<S>> {
        let links_mutation = mutation.links();
        let names_mutation = mutation.names();

        let mut memo = MemoIpld::branch_from(cid, store).await?;
        let mut sphere = store.load::<DagCborCodec, SphereIpld>(&memo.body).await?;

        sphere.links = match !links_mutation.changes().is_empty() {
            true => {
                Some(Links::try_apply_with_cid(sphere.links.as_ref(), links_mutation, store).await?)
            }
            false => sphere.links,
        };

        sphere.names = match !names_mutation.changes().is_empty() {
            true => {
                Some(Names::try_apply_with_cid(sphere.names.as_ref(), names_mutation, store).await?)
            }
            false => sphere.names,
        };

        let allowed_ucans_mutation = mutation.allowed_ucans();
        let revoked_ucans_mutation = mutation.revoked_ucans();

        if !allowed_ucans_mutation.changes().is_empty()
            || !revoked_ucans_mutation.changes().is_empty()
        {
            let mut authorization = match sphere.authorization {
                Some(cid) => store.load::<DagCborCodec, AuthorityIpld>(&cid).await?,
                None => AuthorityIpld::try_empty(store).await?,
            };

            if !allowed_ucans_mutation.changes().is_empty() {
                authorization.allowed = AllowedUcans::try_apply_with_cid(
                    Some(&authorization.allowed),
                    allowed_ucans_mutation,
                    store,
                )
                .await?;
            }

            if !revoked_ucans_mutation.changes().is_empty() {
                authorization.revoked = RevokedUcans::try_apply_with_cid(
                    Some(&authorization.revoked),
                    revoked_ucans_mutation,
                    store,
                )
                .await?;
            }

            sphere.authorization = Some(store.save::<DagCborCodec, _>(&authorization).await?);
        }

        memo.body = store.save::<DagCborCodec, _>(&sphere).await?;

        Ok(SphereRevision {
            memo,
            store: store.clone(),
        })
    }

    pub async fn try_rebase(&self, onto: &Cid) -> Result<SphereRevision<S>> {
        Sphere::try_rebase_with_cid(self.cid(), onto, &mut self.store.clone()).await
    }

    pub async fn try_rebase_with_cid(
        cid: &Cid,
        onto: &Cid,
        store: &mut S,
    ) -> Result<SphereRevision<S>> {
        Sphere::try_apply_mutation_with_cid(
            onto,
            &Sphere::at(cid, store).try_derive_mutation().await?,
            store,
        )
        .await
    }

    /// "Hydrate" a range of revisions of a sphere. See the comments on
    /// the `try_hydrate` method for details and implications.
    pub async fn try_hydrate_range(from: Option<&Cid>, to: &Cid, store: &S) -> Result<()> {
        let timeline = Timeline::new(store);
        let timeslice = timeline.slice(to, from);
        let items = timeslice.try_to_chronological().await?;

        for (cid, _) in items {
            Sphere::at(&cid, store).try_hydrate().await?;
        }

        Ok(())
    }

    /// Attempt to "hydrate" the sphere at the current revision by replaying all
    /// of the changes that were made according to the sphere's changelogs. This
    /// is necessary if the blocks of the sphere were retrieved using sparse
    /// synchronization in order to ensure that interstitial nodes in the various
    /// versioned maps (which are each backed by a HAMT) are populated.
    pub async fn try_hydrate(&self) -> Result<()> {
        Sphere::try_hydrate_with_cid(self.cid(), &mut self.store.clone()).await
    }

    /// Same as try_hydrate, but specifying the CID to hydrate at
    pub async fn try_hydrate_with_cid(cid: &Cid, store: &mut S) -> Result<()> {
        let sphere = Sphere::at(cid, store);
        let memo = sphere.try_as_memo().await?;
        let base_cid = match memo.parent {
            Some(cid) => cid,
            None => {
                let mut base_sphere = SphereIpld::default();
                base_sphere.identity = sphere.try_get_identity().await?;
                let empty_dag = MemoIpld::for_body(store, &base_sphere).await?;
                store.save::<DagCborCodec, _>(&empty_dag).await?
            }
        };

        let hydrated_revision = Sphere::try_apply_mutation_with_cid(
            &base_cid,
            &sphere.try_derive_mutation().await?,
            store,
        )
        .await?;

        if hydrated_revision.memo.body != memo.body {
            return Err(anyhow!(
                "Unexpected CID after hydration (expected: {}, actual: {})",
                memo.body,
                hydrated_revision.memo.body
            ));
        }

        Ok(())
    }

    /// Attempt to linearize the canonical history of the sphere by re-basing
    /// the history onto a branch with an implicitly common lineage.
    pub async fn try_sync<Credential: KeyMaterial>(
        &self,
        old_base: &Cid,
        new_base: &Cid,
        credential: &Credential,
        authorization: Option<&Authorization>,
    ) -> Result<Cid> {
        let mut store = self.store.clone();

        Sphere::try_hydrate_range(Some(old_base), new_base, &self.store).await?;

        let timeline = Timeline::new(&self.store);
        let timeslice = timeline.slice(self.cid(), Some(old_base));
        let rebase_revisions = timeslice.try_to_chronological().await?;

        let mut next_base = *new_base;

        for (cid, _) in rebase_revisions.iter().skip(1) {
            let mut revision = Sphere::try_rebase_with_cid(cid, &next_base, &mut store).await?;
            next_base = revision.try_sign(credential, authorization).await?;
        }

        Ok(next_base)
    }

    /// Generate a new sphere and assign a DID as its owner. The returned tuple
    /// includes the UCAN authorization that enables the owner to manage the
    /// the sphere, as well as a mnemonic string that should be stored side-band
    /// by the owner for the case that they wish to transfer ownership (e.g., if
    /// key rotation is called for).
    pub async fn try_generate(
        owner_did: &str,
        store: &mut S,
    ) -> Result<(Sphere<S>, Authorization, String)> {
        let sphere_key = generate_ed25519_key();
        let mnemonic = ed25519_key_to_mnemonic(&sphere_key)?;
        let sphere_did = Did(sphere_key.get_did().await?);
        let mut memo = MemoIpld::for_body(
            store,
            &SphereIpld {
                identity: sphere_did.clone(),
                links: None,
                names: None,
                sealed: None,
                authorization: None,
            },
        )
        .await?;

        memo.headers.push((
            Header::ContentType.to_string(),
            ContentType::Sphere.to_string(),
        ));

        memo.headers
            .push((Header::Version.to_string(), Version::V0.to_string()));

        let capability = Capability {
            with: With::Resource {
                kind: Resource::Scoped(SphereReference {
                    did: sphere_did.to_string(),
                }),
            },
            can: SphereAction::Authorize,
        };

        let ucan = UcanBuilder::default()
            .issued_by(&sphere_key)
            .for_audience(owner_did)
            .with_lifetime(SPHERE_LIFETIME)
            .claiming_capability(&capability)
            .build()?
            .sign()
            .await?;

        memo.sign(&sphere_key, None).await?;

        let sphere_cid = store.save::<DagCborCodec, _>(&memo).await?;

        let jwt = ucan.encode()?;
        let delegation = DelegationIpld::try_register("(OWNER)", &jwt, store).await?;

        let sphere = Sphere::at(&sphere_cid, store);
        let mut mutation = SphereMutation::new(&sphere_did);
        mutation
            .allowed_ucans_mut()
            .set(&CidKey(delegation.jwt), &delegation);

        let mut revision = sphere.try_apply_mutation(&mutation).await?;
        let sphere_cid = revision.try_sign(&sphere_key, None).await?;

        Ok((
            Sphere::at(&sphere_cid, store),
            Authorization::Ucan(ucan),
            mnemonic,
        ))
    }

    /// Change ownership of the sphere, producing a new UCAN authorization for
    /// the new owner and registering a revocation of the previous owner's
    /// authorization within the sphere.
    pub async fn try_change_owner(
        &self,
        mnemonic: &str,
        next_owner_did: &str,
        current_authorization: &Authorization,
        did_parser: &mut DidParser,
    ) -> Result<(Sphere<S>, Authorization)> {
        let memo = self.store.load::<DagCborCodec, MemoIpld>(&self.cid).await?;
        let sphere = self
            .store
            .load::<DagCborCodec, SphereIpld>(&memo.body)
            .await?;
        let sphere_did = sphere.identity;
        let restored_key = restore_ed25519_key(mnemonic)?;
        let restored_did = restored_key.get_did().await?;

        if sphere_did != restored_did {
            return Err(anyhow!("Incorrect mnemonic provided"));
        }

        let ucan_store = UcanStore(self.store.clone());

        let proof_chain = match current_authorization {
            Authorization::Ucan(ucan) => {
                ProofChain::from_ucan(ucan.clone(), did_parser, &ucan_store).await?
            }
            Authorization::Cid(cid) => {
                ProofChain::try_from_token_string(
                    &ucan_store.require_token(cid).await?,
                    did_parser,
                    &ucan_store,
                )
                .await?
            }
        };

        let authorize_capability = Capability {
            with: With::Resource {
                kind: Resource::Scoped(SphereReference {
                    did: sphere_did.to_string(),
                }),
            },
            can: SphereAction::Authorize,
        };

        let mut proof_is_valid = false;

        for info in proof_chain.reduce_capabilities(&SPHERE_SEMANTICS) {
            if info.capability.enables(&authorize_capability)
                && info.originators.contains(sphere_did.as_str())
            {
                proof_is_valid = true;
                break;
            }
        }

        if !proof_is_valid {
            return Err(anyhow!(
                "Proof does not enable authorizing other identities"
            ));
        }

        let current_jwt_cid = Cid::try_from(current_authorization)?;
        let revocation = RevocationIpld::try_revoke(&current_jwt_cid, &restored_key).await?;

        let ucan = UcanBuilder::default()
            .issued_by(&restored_key)
            .for_audience(next_owner_did)
            .with_lifetime(SPHERE_LIFETIME)
            .claiming_capability(&authorize_capability)
            .build()?
            .sign()
            .await?;

        let jwt = ucan.encode()?;
        let delegation = DelegationIpld::try_register("(OWNER)", &jwt, &self.store).await?;

        let mut mutation = SphereMutation::new(&sphere_did);
        mutation
            .allowed_ucans_mut()
            .set(&CidKey(delegation.jwt), &delegation);
        mutation
            .revoked_ucans_mut()
            .set(&CidKey(current_jwt_cid), &revocation);

        let mut revision = self.try_apply_mutation(&mutation).await?;
        let sphere_cid = revision.try_sign(&restored_key, None).await?;

        Ok((
            Sphere::at(&sphere_cid, &self.store),
            Authorization::Ucan(ucan),
        ))
    }
}

#[cfg(test)]
mod tests {
    use cid::Cid;
    use libipld_core::raw::RawCodec;
    use serde_bytes::Bytes;
    use ucan::{
        builder::UcanBuilder,
        capability::{Capability, Resource, With},
        crypto::{did::DidParser, KeyMaterial},
    };

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use crate::{
        authority::{
            ed25519_key_to_mnemonic, generate_ed25519_key, Authorization, SphereAction,
            SphereReference, SUPPORTED_KEYS,
        },
        data::{AddressIpld, Bundle, CidKey, DelegationIpld, RevocationIpld},
        view::{Sphere, SphereMutation, Timeline, SPHERE_LIFETIME},
    };

    use noosphere_storage::{
        interface::{BlockStore, Store},
        memory::MemoryStore,
        ucan::UcanStore,
    };

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_be_generated_and_later_restored() {
        let mut store = MemoryStore::default();

        let (sphere_cid, sphere_identity) = {
            let owner_key = generate_ed25519_key();
            let owner_did = owner_key.get_did().await.unwrap();
            let (sphere, _, _) = Sphere::try_generate(&owner_did, &mut store).await.unwrap();

            (*sphere.cid(), sphere.try_get_identity().await.unwrap())
        };

        let restored_sphere = Sphere::at(&sphere_cid, &store);
        let restored_identity = restored_sphere.try_get_identity().await.unwrap();

        assert_eq!(sphere_identity, restored_identity);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_includes_the_owner_in_the_list_of_authorizations() {
        let mut store = MemoryStore::default();

        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();
        let (sphere, ucan, _) = Sphere::try_generate(&owner_did, &mut store).await.unwrap();

        let ucan_jwt_cid = Cid::try_from(ucan).unwrap();

        let authorization = sphere.try_get_authority().await.unwrap();
        let allowed_ucans = authorization.try_get_allowed_ucans().await.unwrap();
        let authorization = allowed_ucans.get(&CidKey(ucan_jwt_cid)).await.unwrap();

        assert_eq!(
            authorization,
            Some(&DelegationIpld {
                name: String::from("(OWNER)"),
                jwt: ucan_jwt_cid
            })
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_may_authorize_a_different_key_after_being_created() {
        let mut store = MemoryStore::default();

        let (sphere, authorization, mnemonic) = {
            let owner_key = generate_ed25519_key();
            let owner_did = owner_key.get_did().await.unwrap();
            Sphere::try_generate(&owner_did, &mut store).await.unwrap()
        };

        let next_owner_key = generate_ed25519_key();
        let next_owner_did = next_owner_key.get_did().await.unwrap();

        let mut did_parser = DidParser::new(SUPPORTED_KEYS);
        let (_, new_authorization) = sphere
            .try_change_owner(&mnemonic, &next_owner_did, &authorization, &mut did_parser)
            .await
            .unwrap();

        let ucan_store = UcanStore(store);
        let ucan = authorization.resolve_ucan(&ucan_store).await.unwrap();
        let new_ucan = new_authorization.resolve_ucan(&ucan_store).await.unwrap();

        assert_ne!(ucan.audience(), new_ucan.audience());
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_delegates_to_a_new_owner_and_revokes_the_old_delegation() {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (sphere, original_authorization, mnemonic) =
            { Sphere::try_generate(&owner_did, &mut store).await.unwrap() };

        let sphere_identity = sphere.try_get_identity().await.unwrap();

        let next_owner_key = generate_ed25519_key();
        let next_owner_did = next_owner_key.get_did().await.unwrap();

        let mut did_parser = DidParser::new(SUPPORTED_KEYS);
        let (sphere, new_authorization) = sphere
            .try_change_owner(
                &mnemonic,
                &next_owner_did,
                &original_authorization,
                &mut did_parser,
            )
            .await
            .unwrap();

        let original_jwt_cid = Cid::try_from(&original_authorization).unwrap();
        let new_jwt_cid = Cid::try_from(&new_authorization).unwrap();

        let authority = sphere.try_get_authority().await.unwrap();

        let allowed_ucans = authority.try_get_allowed_ucans().await.unwrap();
        let revoked_ucans = authority.try_get_revoked_ucans().await.unwrap();

        let new_delegation = allowed_ucans.get(&CidKey(new_jwt_cid)).await.unwrap();
        let new_revocation = revoked_ucans.get(&CidKey(original_jwt_cid)).await.unwrap();

        assert_eq!(
            new_delegation,
            Some(&DelegationIpld {
                name: "(OWNER)".into(),
                jwt: new_jwt_cid
            })
        );

        assert!(new_revocation.is_some());

        let new_revocation = new_revocation.unwrap();

        assert_eq!(new_revocation.iss, sphere.try_get_identity().await.unwrap());
        assert_eq!(new_revocation.revoke, original_jwt_cid.to_string());

        let sphere_key = did_parser.parse(&sphere_identity).unwrap();

        new_revocation.try_verify(&sphere_key).await.unwrap();
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
            .try_change_owner(&incorrect_mnemonic, &next_owner_did, &ucan, &mut did_parser)
            .await;

        assert!(authorize_result.is_err());
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_wont_authorize_a_different_key_if_the_proof_does_not_authorize_it() {
        let mut store = MemoryStore::default();

        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();
        let (sphere, authorization, mnemonic) =
            Sphere::try_generate(&owner_did, &mut store).await.unwrap();

        let next_owner_key = generate_ed25519_key();
        let next_owner_did = next_owner_key.get_did().await.unwrap();

        let ucan = authorization.resolve_ucan(&UcanStore(store)).await.unwrap();

        let insufficient_authorization = Authorization::Ucan(
            UcanBuilder::default()
                .issued_by(&owner_key)
                .for_audience(&next_owner_did)
                .with_lifetime(SPHERE_LIFETIME)
                .claiming_capability(&Capability {
                    with: With::Resource {
                        kind: Resource::Scoped(SphereReference {
                            did: sphere.try_get_identity().await.unwrap().to_string(),
                        }),
                    },
                    can: SphereAction::Publish,
                })
                .witnessed_by(&ucan)
                .build()
                .unwrap()
                .sign()
                .await
                .unwrap(),
        );

        let mut did_parser = DidParser::new(SUPPORTED_KEYS);
        let authorize_result = sphere
            .try_change_owner(
                &mnemonic,
                &next_owner_did,
                &insufficient_authorization,
                &mut did_parser,
            )
            .await;

        assert!(authorize_result.is_err());
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_assign_a_link_and_later_read_the_link() {
        let mut store = MemoryStore::default();
        let foo_cid = store.save::<RawCodec, _>(Bytes::new(b"foo")).await.unwrap();
        let foo_key = String::from("foo");

        let sphere_cid = {
            let owner_key = generate_ed25519_key();
            let owner_did = owner_key.get_did().await.unwrap();
            let (sphere, ucan, _) = Sphere::try_generate(&owner_did, &mut store).await.unwrap();

            let mut mutation = SphereMutation::new(&owner_did);
            mutation.links_mut().set(&foo_key, &foo_cid);

            let mut revision = sphere.try_apply_mutation(&mutation).await.unwrap();
            revision.try_sign(&owner_key, Some(&ucan)).await.unwrap()
        };

        let restored_sphere = Sphere::at(&sphere_cid, &store);
        let restored_links = restored_sphere.try_get_links().await.unwrap();
        let restored_foo_cid = restored_links.get(&foo_key).await.unwrap().unwrap();

        assert_eq!(&foo_cid, restored_foo_cid);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_creates_a_lineage_as_changes_are_saved() {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (mut sphere, ucan, _) = Sphere::try_generate(&owner_did, &mut store).await.unwrap();
        let mut lineage = vec![*sphere.cid()];
        let foo_key = String::from("foo");

        for i in 0..2u8 {
            let mut mutation = SphereMutation::new(&owner_did);
            mutation.links_mut().set(
                &foo_key,
                &store.save::<RawCodec, _>(Bytes::new(&[i])).await.unwrap(),
            );
            let mut revision = sphere.try_apply_mutation(&mutation).await.unwrap();
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

        let foo_key = String::from("foo");
        let bar_key = String::from("bar");
        let baz_key = String::from("baz");

        let bar_cid = store.save::<RawCodec, _>(Bytes::new(b"bar")).await.unwrap();
        let baz_cid = store.save::<RawCodec, _>(Bytes::new(b"baz")).await.unwrap();
        let foobar_cid = store
            .save::<RawCodec, _>(Bytes::new(b"foobar"))
            .await
            .unwrap();
        let flurb_cid = store
            .save::<RawCodec, _>(Bytes::new(b"flurb"))
            .await
            .unwrap();

        let mut base_mutation = SphereMutation::new(&owner_did);
        base_mutation.links_mut().set(&foo_key, &bar_cid);

        let mut base_revision = sphere.try_apply_mutation(&base_mutation).await.unwrap();

        let base_cid = base_revision
            .try_sign(&owner_key, Some(&ucan))
            .await
            .unwrap();

        let mut lineage_a_mutation = SphereMutation::new(&owner_did);
        lineage_a_mutation.links_mut().set(&bar_key, &baz_cid);

        let mut lineage_a_revision =
            Sphere::try_apply_mutation_with_cid(&base_cid, &lineage_a_mutation, &mut store)
                .await
                .unwrap();
        let lineage_a_cid = lineage_a_revision
            .try_sign(&owner_key, Some(&ucan))
            .await
            .unwrap();

        let mut lineage_b_mutation = SphereMutation::new(&owner_did);
        lineage_b_mutation.links_mut().set(&foo_key, &foobar_cid);
        lineage_b_mutation.links_mut().set(&baz_key, &flurb_cid);

        let mut lineage_b_revision =
            Sphere::try_apply_mutation_with_cid(&base_cid, &lineage_b_mutation, &mut store)
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

        assert_eq!(
            rebased_links.get(&foo_key).await.unwrap(),
            Some(&foobar_cid)
        );
        assert_eq!(rebased_links.get(&bar_key).await.unwrap(), Some(&baz_cid));
        assert_eq!(rebased_links.get(&baz_key).await.unwrap(), Some(&flurb_cid));
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_hydrate_revisions_of_names_changes() {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (mut sphere, authorization, _) =
            Sphere::try_generate(&owner_did, &mut store).await.unwrap();

        let address = AddressIpld {
            identity: owner_did.clone().into(),
            last_known_record: None,
        };

        let mut mutation = SphereMutation::new(&owner_did);

        mutation.names_mut().set(&"foo".into(), &address);

        let mut revision = sphere.try_apply_mutation(&mutation).await.unwrap();
        let next_cid = revision
            .try_sign(&owner_key, Some(&authorization))
            .await
            .unwrap();

        sphere = Sphere::at(&next_cid, &store);

        let mut mutation = SphereMutation::new(&owner_did);

        mutation.names_mut().set(&"bar".into(), &address);

        let mut revision = sphere.try_apply_mutation(&mutation).await.unwrap();
        let next_cid = revision
            .try_sign(&owner_key, Some(&authorization))
            .await
            .unwrap();

        sphere = Sphere::at(&next_cid, &store);

        let bundle = sphere.try_bundle_until_ancestor(None).await.unwrap();
        let mut other_store = MemoryStore::default();

        bundle.load_into(&mut other_store).await.unwrap();

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
    async fn it_can_hydrate_revisions_from_sparse_link_blocks() {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (mut sphere, ucan, _) = Sphere::try_generate(&owner_did, &mut store).await.unwrap();

        for i in 0..32u8 {
            let mut mutation = SphereMutation::new(&owner_did);
            let key = format!("key{}", i);

            mutation.links_mut().set(
                &key,
                &store.save::<RawCodec, _>(Bytes::new(&[i])).await.unwrap(),
            );
            let mut revision = sphere.try_apply_mutation(&mutation).await.unwrap();
            let next_cid = revision.try_sign(&owner_key, Some(&ucan)).await.unwrap();
            sphere = Sphere::at(&next_cid, &store);
        }

        let bundle = sphere.try_bundle_until_ancestor(None).await.unwrap();
        let mut other_store = MemoryStore::default();

        bundle.load_into(&mut other_store).await.unwrap();

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
    async fn it_can_hydrate_revisions_of_authorization_changes() {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (mut sphere, authorization, _) =
            Sphere::try_generate(&owner_did, &mut store).await.unwrap();

        let ucan = authorization
            .resolve_ucan(&UcanStore(store.clone()))
            .await
            .unwrap();

        let delegation = DelegationIpld::try_register("Test", &ucan.encode().unwrap(), &store)
            .await
            .unwrap();

        let mut mutation = SphereMutation::new(&owner_did);

        mutation
            .allowed_ucans_mut()
            .set(&CidKey(delegation.jwt), &delegation);

        let mut revision = sphere.try_apply_mutation(&mutation).await.unwrap();
        let next_cid = revision
            .try_sign(&owner_key, Some(&authorization))
            .await
            .unwrap();

        sphere = Sphere::at(&next_cid, &store);

        let mut mutation = SphereMutation::new(&owner_did);

        mutation.revoked_ucans_mut().set(
            &CidKey(delegation.jwt),
            &RevocationIpld::try_revoke(&delegation.jwt, &owner_key)
                .await
                .unwrap(),
        );

        let mut revision = sphere.try_apply_mutation(&mutation).await.unwrap();
        let next_cid = revision
            .try_sign(&owner_key, Some(&authorization))
            .await
            .unwrap();

        sphere = Sphere::at(&next_cid, &store);

        let bundle = sphere.try_bundle_until_ancestor(None).await.unwrap();
        let mut other_store = MemoryStore::default();

        bundle.load_into(&mut other_store).await.unwrap();

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
            authorization: &Authorization,
            store: &mut Storage,
            (change_key, change_cid): (&str, &Cid),
        ) -> anyhow::Result<Cid> {
            let mut mutation = SphereMutation::new(author_did);
            mutation.links_mut().set(&change_key.into(), change_cid);

            let mut base_revision =
                Sphere::try_apply_mutation_with_cid(base_cid, &mutation, store).await?;

            base_revision
                .try_sign(credential, Some(authorization))
                .await
        }

        let foo_cid = store.save::<RawCodec, _>(Bytes::new(b"foo")).await.unwrap();
        let bar_cid = store.save::<RawCodec, _>(Bytes::new(b"bar")).await.unwrap();
        let baz_cid = store.save::<RawCodec, _>(Bytes::new(b"baz")).await.unwrap();
        let foobar_cid = store
            .save::<RawCodec, _>(Bytes::new(b"foobar"))
            .await
            .unwrap();
        let flurb_cid = store
            .save::<RawCodec, _>(Bytes::new(b"flurb"))
            .await
            .unwrap();

        let (sphere, authorization, _) =
            Sphere::try_generate(&owner_did, &mut store).await.unwrap();

        let base_cid = make_revision(
            sphere.cid(),
            &owner_did,
            &owner_key,
            &authorization,
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
            &authorization,
            &mut external_store,
            ("bar", &bar_cid),
        )
        .await
        .unwrap();

        let external_cid_b = make_revision(
            &external_cid_a,
            &owner_did,
            &owner_key,
            &authorization,
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
            &authorization,
            &mut store,
            ("baz", &baz_cid),
        )
        .await
        .unwrap();

        let local_cid_b = make_revision(
            &local_cid_a,
            &owner_did,
            &owner_key,
            &authorization,
            &mut store,
            ("bar", &flurb_cid),
        )
        .await
        .unwrap();

        external_bundle.load_into(&mut store).await.unwrap();

        let local_sphere = Sphere::at(&local_cid_b, &store);

        local_sphere
            .try_sync(&base_cid, &external_cid_b, &owner_key, Some(&authorization))
            .await
            .unwrap();
    }
}
