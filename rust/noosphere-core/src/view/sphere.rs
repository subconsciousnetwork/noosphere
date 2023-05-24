use anyhow::{anyhow, Result};
use async_stream::try_stream;
use cid::Cid;
use futures::Stream;
use libipld_cbor::DagCborCodec;
use tokio::sync::OnceCell;
use tokio_stream::StreamExt;

use ucan::{
    builder::UcanBuilder,
    capability::{Capability, Resource, With},
    chain::ProofChain,
    crypto::{did::DidParser, KeyMaterial},
    store::UcanJwtStore,
    Ucan,
};

use crate::{
    authority::{
        ed25519_key_to_mnemonic, generate_capability, generate_ed25519_key, restore_ed25519_key,
        Authorization, SphereAction, SphereReference, SPHERE_SEMANTICS, SUPPORTED_KEYS,
    },
    data::{
        Bundle, ChangelogIpld, ContentType, DelegationIpld, Did, Header, IdentityIpld, Link,
        MapOperation, MemoIpld, RevocationIpld, SphereIpld, TryBundle, Version,
    },
    view::{Content, SphereMutation, SphereRevision, Timeline},
};

use noosphere_storage::{base64_decode, block_serialize, BlockStore, SphereDb, Storage, UcanStore};

use super::{address::AddressBook, Authority, Delegations, Identities, Revocations, Timeslice};

pub const SPHERE_LIFETIME: u64 = 315360000000; // 10,000 years (arbitrarily high)

/// High-level Sphere I/O
#[derive(Clone)]
pub struct Sphere<S: BlockStore> {
    store: S,
    cid: Link<MemoIpld>,
    body: OnceCell<SphereIpld>,
    memo: OnceCell<MemoIpld>,
}

impl<S> Sphere<SphereDb<S>>
where
    S: Storage,
{
    /// The same as [Sphere::traverse_by_petname], but accepts a linear sequence
    /// of petnames and attempts to recursively traverse through spheres using
    /// that sequence. The sequence is traversed from back to front. So, if the
    /// sequence is "gold", "cat", "bob", it will traverse to bob, then to bob's
    /// cat, then to bob's cat's gold.
    pub async fn traverse_by_petnames<F, Fut>(
        &self,
        petname_path: &[String],
        replicate: &F,
    ) -> Result<Option<Self>>
    where
        F: Fn(Link<MemoIpld>, Option<Link<MemoIpld>>) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let mut sphere: Option<Self> = None;
        let mut path = Vec::from(petname_path);
        let mut traversed = Vec::new();

        while let Some(petname) = path.pop() {
            let next_sphere = match sphere {
                None => self.traverse_by_petname(&petname, replicate).await?,
                Some(sphere) => sphere.traverse_by_petname(&petname, replicate).await?,
            };
            sphere = match next_sphere {
                any @ Some(_) => {
                    traversed.push(petname);
                    any
                }
                None => {
                    warn!(
                        "No sphere found for '{petname}' after traveling to '{}'",
                        traversed.join(" -> ")
                    );
                    return Ok(None);
                }
            };
        }

        Ok(sphere)
    }

    /// Given a petname that has been assigned to a sphere identity within this
    /// sphere's address book, produce a [Sphere] backed by the same storage as
    /// this one, but that accesses the sphere referred to by the designated
    /// [Did] identity. If the local data for the sphere being traversed to is
    /// not available, an attempt will be made to replicate the data from a
    /// Noosphere Gateway.
    pub async fn traverse_by_petname<F, Fut>(
        &self,
        petname: &str,
        replicate: &F,
    ) -> Result<Option<Self>>
    where
        F: Fn(Link<MemoIpld>, Option<Link<MemoIpld>>) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        // Resolve petname to sphere version via address book entry

        let identity = match self
            .get_address_book()
            .await?
            .get_identities()
            .await?
            .get(&petname.to_string())
            .await?
        {
            Some(address) => address.clone(),
            None => {
                warn!("\"{petname}\" is not assigned to an identity");
                return Ok(None);
            }
        };

        debug!("Petname assigned to {:?}", identity);

        let link_record_version = match identity.link_record(&UcanStore(self.store().clone())).await
        {
            Some(link_record) => link_record.get_link(),
            None => None,
        };

        let link_record_version = match link_record_version {
            Some(cid) => cid,
            None => {
                return Err(anyhow!(
                    "No version has been resolved for \"{petname}\" ({})",
                    identity.did
                ));
            }
        };

        debug!("Link record version is {}", link_record_version);

        // Check for version in local sphere DB
        // If desired version available, check for memo and body blocks

        let local_version = self
            .store()
            .get_version(&identity.did)
            .await?
            .map(|cid| cid.into());
        let (replication_required, local_version) =
            if local_version.as_ref() == Some(&link_record_version) {
                match self
                    .store()
                    .load::<DagCborCodec, MemoIpld>(&link_record_version)
                    .await
                {
                    Ok(memo) => {
                        if memo.content_type() != Some(ContentType::Sphere) {
                            return Err(anyhow!(
                            "Resolved content for \"{petname}\" ({}) does not refer to a sphere",
                            identity.did
                        ));
                        }

                        debug!("Checking to see if we can get the sphere body...");

                        match self
                            .store()
                            .load::<DagCborCodec, SphereIpld>(&memo.body)
                            .await
                        {
                            Ok(_) => (false, local_version),
                            Err(error) => {
                                warn!("{error}");
                                (true, None)
                            }
                        }
                    }
                    Err(error) => {
                        warn!("{error}");
                        (true, None)
                    }
                }
            } else {
                (true, local_version)
            };

        // If no version available or memo/body missing, attempt to replicate the needed blocks

        if replication_required {
            debug!("Attempting to replicate from gateway...");

            replicate(link_record_version.clone(), local_version).await?;
        }

        Ok(Some(Sphere::at(&link_record_version, self.store())))
    }
}

impl<S: BlockStore> Sphere<S> {
    /// Initialize a [Sphere] at the given [Cid] version; even though a
    /// version and [BlockStore] are provided, the initialized [Sphere] is
    /// lazy and won't load the associated [MemoIpld] or [SphereIpld] unless
    /// they are accessed.
    pub fn at(cid: &Link<MemoIpld>, store: &S) -> Sphere<S> {
        Sphere {
            store: store.clone(),
            cid: cid.clone(),
            body: OnceCell::new(),
            memo: OnceCell::new(),
        }
    }

    /// Given a memo that refers to a [SphereIpld] body, compute the memo's
    /// [Cid] and initialize a [Sphere] for it.
    pub fn from_memo(memo: &MemoIpld, store: &S) -> Result<Sphere<S>> {
        let (cid, _) = block_serialize::<DagCborCodec, _>(memo)?;
        Ok(Sphere {
            store: store.clone(),
            cid: cid.into(),
            body: OnceCell::new(),
            memo: OnceCell::new_with(Some(memo.clone())),
        })
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    /// Get the CID that points to the sphere's wrapping memo that corresponds
    /// to this revision of the sphere
    pub fn cid(&self) -> &Link<MemoIpld> {
        &self.cid
    }

    /// Load the wrapping [MemoIpld] of the sphere data
    pub async fn to_memo(&self) -> Result<MemoIpld> {
        Ok(self
            .memo
            .get_or_try_init(|| async { self.store.load::<DagCborCodec, _>(&self.cid).await })
            .await?
            .clone())
    }

    /// Load the body [SphereIpld] of this sphere
    pub async fn to_body(&self) -> Result<SphereIpld> {
        Ok(self
            .body
            .get_or_try_init(|| async {
                self.store
                    .load::<DagCborCodec, _>(&self.to_memo().await?.body)
                    .await
            })
            .await?
            .clone())
    }

    /// Produce a bundle that contains the sparse set of blocks needed
    /// to produce this revision to the sphere
    pub async fn to_bundle(&self) -> Result<Bundle> {
        MemoIpld::bundle_with_cid(self.cid(), &self.store).await
    }

    /// Produce a bundle that contains the sparse set of blocks needed to
    /// produce a series of sequential revisions of this sphere, up to but
    /// excluding the given [Cid] (or until the genesis revision of the
    /// sphere if no [Cid] is given).
    pub async fn bundle_until_ancestor(&self, cid: Option<&Link<MemoIpld>>) -> Result<Bundle> {
        Bundle::from_timeslice(
            &Timeline::new(&self.store).slice(&self.cid, cid),
            &self.store,
        )
        .await
    }

    /// Get a [Sphere] view over the parent revision of the sphere relative to
    /// this revision, if one exists
    pub async fn get_parent(&self) -> Result<Option<Sphere<S>>> {
        match self.to_memo().await?.parent {
            Some(cid) => Ok(Some(Sphere::at(&cid, &self.store))),
            None => Ok(None),
        }
    }

    /// Attempt to load the [Links] of this sphere. If no links have been
    /// set for this sphere yet, this initializes an empty [Links] and returns
    /// it for the caller to populate.
    pub async fn get_content(&self) -> Result<Content<S>> {
        let sphere = self.to_body().await?;

        Ok(Content::at(&sphere.content, &self.store.clone()))
    }

    /// Attempt to load the [Authority] of this sphere. If no authorizations or
    /// revocations have been added to this sphere yet, this initializes an
    /// empty [Authority] and returns it for the caller to populate.
    pub async fn get_authority(&self) -> Result<Authority<S>> {
        let sphere = self.to_body().await?;

        Ok(Authority::at(&sphere.authority, &self.store.clone()))
    }

    pub async fn get_address_book(&self) -> Result<AddressBook<S>> {
        let sphere = self.to_body().await?;

        Ok(AddressBook::at(&sphere.address_book, &self.store.clone()))
    }

    /// Get the [Did] identity of the sphere
    pub async fn get_identity(&self) -> Result<Did> {
        let sphere = self.to_body().await?;

        Ok(sphere.identity)
    }

    /// Derive the mutation that would be required to produce the current
    /// sphere revision given its immediate ancestral parent. Note that
    /// this only considers changes that are internal to the sphere, and is
    /// not inclusive of the headers of the sphere's memo.
    pub async fn derive_mutation(&self) -> Result<SphereMutation> {
        // TODO: This routine can probably be broken out into a trait
        // implementation on our views
        let memo = self.to_memo().await?;
        let author = memo
            .get_first_header(&Header::Author)
            .ok_or_else(|| anyhow!("No author header found"))?;

        let mut mutation = SphereMutation::new(&author);

        let parent = match self.get_parent().await? {
            Some(parent) => parent,
            None => return Ok(mutation),
        };

        let parent_content = parent.get_content().await?;
        let content = self.get_content().await?;

        if content.cid() != parent_content.cid() {
            let changelog = content.get_changelog().await?;

            if changelog.is_empty() {
                return Err(anyhow!("Content changed but the changelog is empty"));
            }

            mutation.content_mut().apply_changelog(changelog)?;
        }

        let parent_address_book = parent.get_address_book().await?;
        let address_book = self.get_address_book().await?;

        if address_book.cid() != parent_address_book.cid() {
            let parent_identities = parent_address_book.get_identities().await?;
            let identities = address_book.get_identities().await?;

            if identities.cid() != parent_identities.cid() {
                let changelog = identities.get_changelog().await?;

                if changelog.is_empty() {
                    return Err(anyhow!("Identities changed but the changelog is empty"));
                }

                mutation.identities_mut().apply_changelog(changelog)?;
            }
        }

        let parent_authorization = parent.get_authority().await?;
        let authorization = self.get_authority().await?;

        if authorization.cid() != parent_authorization.cid() {
            let parent_delegations = parent_authorization.get_delegations().await?;
            let delegations = authorization.get_delegations().await?;

            if delegations.cid() != parent_delegations.cid() {
                let changelog = delegations.get_changelog().await?;

                if changelog.is_empty() {
                    return Err(anyhow!("Allowed UCANs changed but the changelog is empty"));
                }

                mutation.delegations_mut().apply_changelog(changelog)?;
            }

            let parent_revocations = parent_authorization.get_revocations().await?;
            let revocations = authorization.get_revocations().await?;

            if revocations.cid() != parent_revocations.cid() {
                let changelog = revocations.get_changelog().await?;

                if changelog.is_empty() {
                    return Err(anyhow!("Revoked UCANs changed but the changelog is empty"));
                }

                mutation.revocations_mut().apply_changelog(changelog)?;
            }
        }

        Ok(mutation)
    }

    /// Apply a mutation to the sphere, producing a new sphere revision that
    /// must then be signed as an additional step.
    pub async fn apply_mutation(&self, mutation: &SphereMutation) -> Result<SphereRevision<S>> {
        Sphere::apply_mutation_with_cid(self.cid(), mutation, &mut self.store.clone()).await
    }

    /// Apply a mutation to the sphere given a revision CID, producing a new
    /// sphere revision that must then be signed as an additional step
    pub async fn apply_mutation_with_cid(
        cid: &Link<MemoIpld>,
        mutation: &SphereMutation,
        store: &mut S,
    ) -> Result<SphereRevision<S>> {
        // TODO: This routine can probably be broken out into a trait
        // implementation on our views
        let content_mutation = mutation.content();

        let mut memo = MemoIpld::branch_from(cid, store).await?;
        let mut sphere = store.load::<DagCborCodec, SphereIpld>(&memo.body).await?;

        sphere.content = match !content_mutation.changes().is_empty() {
            true => Content::apply_with_cid(Some(sphere.content), content_mutation, store)
                .await?
                .into(),
            false => sphere.content,
        };

        let identities_mutation = mutation.identities();

        if !identities_mutation.changes().is_empty() {
            let mut address_book = sphere.address_book.load_from(store).await?;

            address_book.identities = Identities::apply_with_cid(
                Some(address_book.identities),
                identities_mutation,
                store,
            )
            .await?
            .into();

            sphere.address_book = store.save::<DagCborCodec, _>(&address_book).await?.into();
        }

        let delegations_mutation = mutation.delegations();
        let revocations_mutation = mutation.revocations();

        if !delegations_mutation.changes().is_empty() || !revocations_mutation.changes().is_empty()
        {
            let mut authority = sphere.authority.load_from(store).await?;

            if !delegations_mutation.changes().is_empty() {
                authority.delegations = Delegations::apply_with_cid(
                    Some(authority.delegations),
                    delegations_mutation,
                    store,
                )
                .await?
                .into();
            }

            if !revocations_mutation.changes().is_empty() {
                authority.revocations = Revocations::apply_with_cid(
                    Some(authority.revocations),
                    revocations_mutation,
                    store,
                )
                .await?
                .into();
            }

            sphere.authority = store.save::<DagCborCodec, _>(&authority).await?.into();
        }

        memo.body = store.save::<DagCborCodec, _>(&sphere).await?;

        Ok(SphereRevision {
            sphere_identity: sphere.identity,
            memo,
            store: store.clone(),
        })
    }

    /// Same as `rebase_version`, but uses the version that this [Sphere] refers
    /// to as the version to rebase.
    pub async fn rebase(&self, onto: &Link<MemoIpld>) -> Result<SphereRevision<S>> {
        Sphere::rebase_version(self.cid(), onto, &mut self.store.clone()).await
    }

    /// "Rebase" the given version of a sphere so that its change is made with
    /// the "onto" version as its base.
    pub async fn rebase_version(
        cid: &Link<MemoIpld>,
        onto: &Link<MemoIpld>,
        store: &mut S,
    ) -> Result<SphereRevision<S>> {
        Sphere::apply_mutation_with_cid(
            onto,
            &Sphere::at(cid, store).derive_mutation().await?,
            store,
        )
        .await
    }

    /// "Hydrate" a range of revisions of a sphere. See the comments on
    /// the `try_hydrate` method for details and implications.
    #[instrument(level = "trace", skip(store))]
    #[deprecated(note = "Use hydrate_timeslice instead")]
    pub async fn hydrate_range(
        from: Option<&Link<MemoIpld>>,
        to: &Link<MemoIpld>,
        store: &S,
    ) -> Result<()> {
        let timeline = Timeline::new(store);
        let timeslice = timeline.slice(to, from);

        Self::hydrate_timeslice(&timeslice).await?;

        Ok(())
    }

    /// "Hydrate" a range of revisions of a sphere, defined by a [Timeslice].
    /// See the comments on the `try_hydrate` method for details and
    /// implications.
    pub async fn hydrate_timeslice<'a>(timeslice: &Timeslice<'a, S>) -> Result<()> {
        let items = timeslice.to_chronological().await?;

        for (cid, _) in items {
            Sphere::at(&cid, timeslice.timeline.store).hydrate().await?;
        }

        Ok(())
    }

    /// Attempt to "hydrate" the sphere at the current revision by replaying all
    /// of the changes that were made according to the sphere's changelogs. This
    /// is necessary if the blocks of the sphere were retrieved using sparse
    /// synchronization in order to ensure that interstitial nodes in the
    /// various versioned maps (which are each backed by a HAMT) are
    /// populated.
    pub async fn hydrate(&self) -> Result<()> {
        Sphere::hydrate_with_cid(self.cid(), &mut self.store.clone()).await
    }

    /// Same as try_hydrate, but specifying the CID to hydrate at
    pub async fn hydrate_with_cid(cid: &Link<MemoIpld>, store: &mut S) -> Result<()> {
        trace!("Hydrating {}...", cid);
        let sphere = Sphere::at(cid, store);
        let memo = sphere.to_memo().await?;
        let base_cid = match memo.parent {
            Some(cid) => cid,
            None => {
                let base_sphere = SphereIpld::new(&sphere.get_identity().await?, store).await?;
                let empty_dag = MemoIpld::for_body(store, &base_sphere).await?;
                store.save::<DagCborCodec, _>(&empty_dag).await?.into()
            }
        };

        let hydrated_revision =
            Sphere::apply_mutation_with_cid(&base_cid, &sphere.derive_mutation().await?, store)
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
    pub async fn sync<Credential: KeyMaterial>(
        &self,
        old_base: &Link<MemoIpld>,
        new_base: &Link<MemoIpld>,
        credential: &Credential,
        authorization: Option<&Authorization>,
    ) -> Result<Link<MemoIpld>> {
        let mut store = self.store.clone();

        let timeline = Timeline::new(&self.store);
        Sphere::hydrate_timeslice(&timeline.slice(new_base, Some(old_base))).await?;

        let timeline = Timeline::new(&self.store);
        let timeslice = timeline.slice(self.cid(), Some(old_base));
        let rebase_revisions = timeslice.to_chronological().await?;

        let mut next_base = new_base.clone();

        for (cid, _) in rebase_revisions.iter().skip(1) {
            let mut revision = Sphere::rebase_version(cid, &next_base, &mut store).await?;
            next_base = revision.sign(credential, authorization).await?;
        }

        Ok(next_base)
    }

    /// Generate a new sphere and assign a DID as its owner. The returned tuple
    /// includes the UCAN authorization that enables the owner to manage the
    /// the sphere, as well as a mnemonic string that should be stored side-band
    /// by the owner for the case that they wish to transfer ownership (e.g., if
    /// key rotation is called for).
    pub async fn generate(
        owner_did: &str,
        store: &mut S,
    ) -> Result<(Sphere<S>, Authorization, String)> {
        let sphere_key = generate_ed25519_key();
        let mnemonic = ed25519_key_to_mnemonic(&sphere_key)?;
        let sphere_did = Did(sphere_key.get_did().await?);
        let sphere = SphereIpld::new(&sphere_did, store).await?;
        let mut memo = MemoIpld::for_body(store, &sphere).await?;

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

        let sphere_cid = store.save::<DagCborCodec, _>(&memo).await?.into();

        let jwt = ucan.encode()?;
        let delegation = DelegationIpld::register("(OWNER)", &jwt, store).await?;

        let sphere = Sphere::at(&sphere_cid, store);
        let mut mutation = SphereMutation::new(&sphere_did);
        mutation
            .delegations_mut()
            .set(&Link::new(delegation.jwt), &delegation);

        memo.sign(&sphere_key, None).await?;
        let mut revision = sphere.apply_mutation(&mutation).await?;
        let sphere_cid = revision.sign(&sphere_key, None).await?;

        Ok((
            Sphere::at(&sphere_cid, store),
            Authorization::Ucan(ucan),
            mnemonic,
        ))
    }

    /// Change ownership of the sphere, producing a new UCAN authorization for
    /// the new owner and registering a revocation of the previous owner's
    /// authorization within the sphere.
    pub async fn change_owner(
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
                ProofChain::from_ucan(ucan.clone(), None, did_parser, &ucan_store).await?
            }
            Authorization::Cid(cid) => {
                ProofChain::try_from_token_string(
                    &ucan_store.require_token(cid).await?,
                    None,
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
        let revocation = RevocationIpld::revoke(&current_jwt_cid, &restored_key).await?;

        let ucan = UcanBuilder::default()
            .issued_by(&restored_key)
            .for_audience(next_owner_did)
            .with_lifetime(SPHERE_LIFETIME)
            .claiming_capability(&authorize_capability)
            .build()?
            .sign()
            .await?;

        let jwt = ucan.encode()?;
        let delegation = DelegationIpld::register("(OWNER)", &jwt, &self.store).await?;

        let mut mutation = SphereMutation::new(&sphere_did);
        mutation
            .delegations_mut()
            .set(&Link::new(delegation.jwt), &delegation);
        mutation
            .revocations_mut()
            .set(&Link::new(current_jwt_cid), &revocation);

        let mut revision = self.apply_mutation(&mutation).await?;
        let sphere_cid = revision.sign(&restored_key, None).await?;

        Ok((
            Sphere::at(&sphere_cid, &self.store),
            Authorization::Ucan(ucan),
        ))
    }

    /// Consume the [Sphere] and get a [Stream] that yields a `(Cid, Sphere)`
    /// tuple for each step in the sphere's history (*excluding* the version
    /// represented by `since`). History is traversed in reverse-chronological
    /// order. If `None` is given for `since`, the entire history of the sphere
    /// will be streamed.
    pub fn into_history_stream(
        self,
        since: Option<&Link<MemoIpld>>,
    ) -> impl Stream<Item = Result<(Link<MemoIpld>, Sphere<S>)>> {
        let since = since.cloned();

        try_stream! {
            let timeline = Timeline::new(&self.store);
            let timeslice = timeline.slice(&self.cid, since.as_ref());
            let stream = timeslice.stream();

            for await item in stream {
                let (cid, memo) = item?;

                match since {
                    Some(since) if since == cid => {
                        break;
                    },
                    _ => ()
                };

                yield (cid, Sphere::from_memo(&memo, &self.store)?);
            }
        }
    }

    // Validate this sphere revision's signature and proof chain
    pub async fn verify_signature(&self) -> Result<()> {
        let memo = self.to_memo().await?;

        // Ensure that we have the correct content type
        memo.expect_header(&Header::ContentType, &ContentType::Sphere)?;

        // Extract signature from the eponimous header
        let signature_header = memo
            .get_header(&Header::Signature)
            .first()
            .cloned()
            .ok_or_else(|| anyhow!("No signature header found"))?;

        let signature = base64_decode(&signature_header)?;

        let mut did_parser = DidParser::new(SUPPORTED_KEYS);

        let sphere_ipld = self.to_body().await?;

        // If we have an authorizing proof...
        if let Some(proof_header) = memo.get_header(&Header::Proof).first() {
            // Interpret the header as a JWT-encoded UCAN..
            let ucan = Ucan::try_from(proof_header.as_str())?;

            // Discover the intended audience of the UCAN
            let credential = did_parser.parse(ucan.audience())?;

            // Verify the audience signature of the body CID
            credential.verify(&memo.body.to_bytes(), &signature).await?;

            // Check the proof's provenance and that it enables the signer to sign
            let ucan_store = UcanStore(self.store.clone());
            let proof = ProofChain::from_ucan(ucan, None, &mut did_parser, &ucan_store).await?;

            let desired_capability = generate_capability(&sphere_ipld.identity, SphereAction::Push);

            for capability_info in proof.reduce_capabilities(&SPHERE_SEMANTICS) {
                let capability = capability_info.capability;
                if capability_info
                    .originators
                    .contains(sphere_ipld.identity.as_str())
                    && capability.enables(&desired_capability)
                {
                    return Ok(());
                }
            }

            Err(anyhow!("Proof did not enable signer to sign this sphere"))
        } else {
            // Assume the identity is the signer
            let credential = did_parser.parse(&sphere_ipld.identity)?;

            // Verify the identity signature of the body CID
            credential.verify(&memo.body.to_bytes(), &signature).await?;

            Ok(())
        }
    }

    /// Consume the [Sphere] and get a [Stream] that yields the [ChangelogIpld]
    /// for petnames and resolutions at each version of the sphere. This stream will
    /// skip versions where no petnames or resolutions changed.
    pub fn into_identities_changelog_stream(
        self,
        since: Option<&Link<MemoIpld>>,
    ) -> impl Stream<
        Item = Result<(
            Link<MemoIpld>,
            ChangelogIpld<MapOperation<String, IdentityIpld>>,
        )>,
    > {
        let since = since.cloned();

        try_stream! {
            let history: Vec<Result<(Link<MemoIpld>, Sphere<S>)>> = self.into_history_stream(since.as_ref()).collect().await;

            for item in history.into_iter().rev() {
                let (cid, sphere) = item?;
                let identities = sphere.get_address_book().await?.get_identities().await?;
                let changelog = identities.load_changelog().await?;

                if !changelog.is_empty() {
                    yield (cid, changelog);
                }
            }
        }
    }

    /// Consume the [Sphere] and get a [Stream] that yields the [ChangelogIpld]
    /// for content slugs at each version of the sphere. This stream will skip
    /// versions where no content changed.
    pub fn into_content_changelog_stream(
        self,
        since: Option<&Link<MemoIpld>>,
    ) -> impl Stream<
        Item = Result<(
            Link<MemoIpld>,
            ChangelogIpld<MapOperation<String, Link<MemoIpld>>>,
        )>,
    > {
        let since = since.cloned();

        try_stream! {
            let history: Vec<Result<(Link<MemoIpld>, Sphere<S>)>> = self.into_history_stream(since.as_ref()).collect().await;

            for item in history.into_iter().rev() {
                let (cid, sphere) = item?;
                let content = sphere.get_content().await?;
                let changelog = content.load_changelog().await?;

                if !changelog.is_empty() {
                    yield (cid, changelog);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use cid::Cid;
    use libipld_cbor::DagCborCodec;
    use tokio_stream::StreamExt;
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
        data::{Bundle, DelegationIpld, IdentityIpld, Link, MemoIpld, RevocationIpld},
        view::{Sphere, SphereMutation, Timeline},
    };

    use noosphere_storage::{BlockStore, MemoryStore, Store, UcanStore};

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_be_generated_and_later_restored() {
        let mut store = MemoryStore::default();

        let (sphere_cid, sphere_identity) = {
            let owner_key = generate_ed25519_key();
            let owner_did = owner_key.get_did().await.unwrap();
            let (sphere, _, _) = Sphere::generate(&owner_did, &mut store).await.unwrap();

            (sphere.cid().clone(), sphere.get_identity().await.unwrap())
        };

        let restored_sphere = Sphere::at(&sphere_cid, &store);
        let restored_identity = restored_sphere.get_identity().await.unwrap();

        assert_eq!(sphere_identity, restored_identity);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_includes_the_owner_in_the_list_of_authorizations() {
        let mut store = MemoryStore::default();

        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();
        let (sphere, ucan, _) = Sphere::generate(&owner_did, &mut store).await.unwrap();

        let ucan_jwt_cid = Cid::try_from(ucan).unwrap();

        let authority = sphere.get_authority().await.unwrap();
        let delegations = authority.get_delegations().await.unwrap();
        let delegation = delegations.get(&Link::new(ucan_jwt_cid)).await.unwrap();

        assert_eq!(
            delegation,
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
            Sphere::generate(&owner_did, &mut store).await.unwrap()
        };

        let next_owner_key = generate_ed25519_key();
        let next_owner_did = next_owner_key.get_did().await.unwrap();

        let mut did_parser = DidParser::new(SUPPORTED_KEYS);
        let (_, new_authorization) = sphere
            .change_owner(&mnemonic, &next_owner_did, &authorization, &mut did_parser)
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
            { Sphere::generate(&owner_did, &mut store).await.unwrap() };

        let sphere_identity = sphere.get_identity().await.unwrap();

        let next_owner_key = generate_ed25519_key();
        let next_owner_did = next_owner_key.get_did().await.unwrap();

        let mut did_parser = DidParser::new(SUPPORTED_KEYS);
        let (sphere, new_authorization) = sphere
            .change_owner(
                &mnemonic,
                &next_owner_did,
                &original_authorization,
                &mut did_parser,
            )
            .await
            .unwrap();

        let original_jwt_cid = Cid::try_from(&original_authorization).unwrap();
        let new_jwt_cid = Cid::try_from(&new_authorization).unwrap();

        let authority = sphere.get_authority().await.unwrap();

        let delegations = authority.get_delegations().await.unwrap();
        let revocations = authority.get_revocations().await.unwrap();

        let new_delegation = delegations.get(&Link::new(new_jwt_cid)).await.unwrap();
        let new_revocation = revocations.get(&Link::new(original_jwt_cid)).await.unwrap();

        assert_eq!(
            new_delegation,
            Some(&DelegationIpld {
                name: "(OWNER)".into(),
                jwt: new_jwt_cid
            })
        );

        assert!(new_revocation.is_some());

        let new_revocation = new_revocation.unwrap();

        assert_eq!(new_revocation.iss, sphere.get_identity().await.unwrap());
        assert_eq!(new_revocation.revoke, original_jwt_cid.to_string());

        let sphere_key = did_parser.parse(&sphere_identity).unwrap();

        new_revocation.verify(&sphere_key).await.unwrap();
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_wont_authorize_a_different_key_if_the_mnemonic_is_wrong() {
        let mut store = MemoryStore::default();

        let (sphere, ucan, _) = {
            let owner_key = generate_ed25519_key();
            let owner_did = owner_key.get_did().await.unwrap();
            Sphere::generate(&owner_did, &mut store).await.unwrap()
        };

        let next_owner_key = generate_ed25519_key();
        let next_owner_did = next_owner_key.get_did().await.unwrap();
        let incorrect_mnemonic = ed25519_key_to_mnemonic(&next_owner_key).unwrap();

        let mut did_parser = DidParser::new(SUPPORTED_KEYS);
        let authorize_result = sphere
            .change_owner(&incorrect_mnemonic, &next_owner_did, &ucan, &mut did_parser)
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
            Sphere::generate(&owner_did, &mut store).await.unwrap();

        let next_owner_key = generate_ed25519_key();
        let next_owner_did = next_owner_key.get_did().await.unwrap();

        let ucan = authorization.resolve_ucan(&UcanStore(store)).await.unwrap();

        let insufficient_authorization = Authorization::Ucan(
            UcanBuilder::default()
                .issued_by(&owner_key)
                .for_audience(&next_owner_did)
                .claiming_capability(&Capability {
                    with: With::Resource {
                        kind: Resource::Scoped(SphereReference {
                            did: sphere.get_identity().await.unwrap().to_string(),
                        }),
                    },
                    can: SphereAction::Publish,
                })
                .witnessed_by(&ucan)
                .with_expiration(*ucan.expires_at())
                .build()
                .unwrap()
                .sign()
                .await
                .unwrap(),
        );

        let mut did_parser = DidParser::new(SUPPORTED_KEYS);
        let authorize_result = sphere
            .change_owner(
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
        // let foo_cid = store.save::<RawCodec, _>(Bytes::new(b"foo")).await.unwrap();
        let foo_memo = MemoIpld::for_body(&mut store, b"foo").await.unwrap();
        let foo_memo_link = store
            .save::<DagCborCodec, _>(&foo_memo)
            .await
            .unwrap()
            .into();
        let foo_key = String::from("foo");

        let sphere_cid = {
            let owner_key = generate_ed25519_key();
            let owner_did = owner_key.get_did().await.unwrap();
            let (sphere, ucan, _) = Sphere::generate(&owner_did, &mut store).await.unwrap();

            let mut mutation = SphereMutation::new(&owner_did);
            mutation.content_mut().set(&foo_key, &foo_memo_link);

            let mut revision = sphere.apply_mutation(&mutation).await.unwrap();
            revision.sign(&owner_key, Some(&ucan)).await.unwrap()
        };

        let restored_sphere = Sphere::at(&sphere_cid, &store);
        let restored_links = restored_sphere.get_content().await.unwrap();
        let restored_foo_memo = restored_links
            .get(&foo_key)
            .await
            .unwrap()
            .unwrap()
            .load_from(&store)
            .await
            .unwrap();

        assert_eq!(foo_memo, restored_foo_memo);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_creates_a_lineage_as_changes_are_saved() {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (mut sphere, ucan, _) = Sphere::generate(&owner_did, &mut store).await.unwrap();
        let mut lineage = vec![sphere.cid().clone()];
        let foo_key = String::from("foo");

        for i in 0..2u8 {
            let mut mutation = SphereMutation::new(&owner_did);
            let memo = MemoIpld::for_body(&mut store, &[i]).await.unwrap();

            mutation.content_mut().set(
                &foo_key,
                &store.save::<DagCborCodec, _>(&memo).await.unwrap().into(),
            );
            let mut revision = sphere.apply_mutation(&mutation).await.unwrap();
            let next_cid = revision.sign(&owner_key, Some(&ucan)).await.unwrap();

            sphere = Sphere::at(&next_cid, &store);
            lineage.push(next_cid);
        }

        assert_eq!(lineage.len(), 3);

        for cid in lineage.iter().rev() {
            assert_eq!(cid, sphere.cid());
            if let Some(parent) = sphere.get_parent().await.unwrap() {
                sphere = parent;
            }
        }
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_excludes_since_when_resolving_changes() {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (mut sphere, ucan, _) = Sphere::generate(&owner_did, &mut store).await.unwrap();
        let mut lineage = vec![sphere.cid().clone()];

        for i in 0..5u8 {
            let mut mutation = SphereMutation::new(&owner_did);
            let memo = MemoIpld::for_body(&mut store, &[i]).await.unwrap();

            mutation.content_mut().set(
                &format!("foo/{i}"),
                &store.save::<DagCborCodec, _>(&memo).await.unwrap().into(),
            );
            let mut revision = sphere.apply_mutation(&mutation).await.unwrap();
            let next_cid = revision.sign(&owner_key, Some(&ucan)).await.unwrap();

            sphere = Sphere::at(&next_cid, &store);
            lineage.push(next_cid);
        }

        let since = lineage[2].clone();

        let stream = sphere.into_content_changelog_stream(Some(&since));

        tokio::pin!(stream);

        let change_revisions = stream
            .fold(Vec::new(), |mut all, next| {
                match next {
                    Ok((cid, _)) => all.push(cid),
                    Err(error) => unreachable!("{}", error),
                };
                all
            })
            .await;

        assert_eq!(change_revisions.len(), 3);
        assert!(!change_revisions.contains(&since));
        assert_eq!(change_revisions, lineage.split_at(3).1);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_rebase_a_change_onto_a_parallel_lineage() {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (sphere, ucan, _) = Sphere::generate(&owner_did, &mut store).await.unwrap();

        let foo_key = String::from("foo");
        let bar_key = String::from("bar");
        let baz_key = String::from("baz");

        let bar_memo = MemoIpld::for_body(&mut store, b"bar").await.unwrap();
        let bar_memo_link = store
            .save::<DagCborCodec, _>(&bar_memo)
            .await
            .unwrap()
            .into();
        let baz_memo = MemoIpld::for_body(&mut store, b"baz").await.unwrap();
        let baz_memo_link = store
            .save::<DagCborCodec, _>(&baz_memo)
            .await
            .unwrap()
            .into();
        let foobar_memo = MemoIpld::for_body(&mut store, b"foobar").await.unwrap();
        let foobar_memo_link = store
            .save::<DagCborCodec, _>(&foobar_memo)
            .await
            .unwrap()
            .into();
        let flurb_memo = MemoIpld::for_body(&mut store, b"flurb").await.unwrap();
        let flurb_memo_link = store
            .save::<DagCborCodec, _>(&flurb_memo)
            .await
            .unwrap()
            .into();

        let mut base_mutation = SphereMutation::new(&owner_did);
        base_mutation.content_mut().set(&foo_key, &bar_memo_link);

        let mut base_revision = sphere.apply_mutation(&base_mutation).await.unwrap();

        let base_cid = base_revision.sign(&owner_key, Some(&ucan)).await.unwrap();

        let mut lineage_a_mutation = SphereMutation::new(&owner_did);
        lineage_a_mutation
            .content_mut()
            .set(&bar_key, &baz_memo_link);

        let mut lineage_a_revision =
            Sphere::apply_mutation_with_cid(&base_cid, &lineage_a_mutation, &mut store)
                .await
                .unwrap();
        let lineage_a_cid = lineage_a_revision
            .sign(&owner_key, Some(&ucan))
            .await
            .unwrap();

        let mut lineage_b_mutation = SphereMutation::new(&owner_did);
        lineage_b_mutation
            .content_mut()
            .set(&foo_key, &foobar_memo_link);
        lineage_b_mutation
            .content_mut()
            .set(&baz_key, &flurb_memo_link);

        let mut lineage_b_revision =
            Sphere::apply_mutation_with_cid(&base_cid, &lineage_b_mutation, &mut store)
                .await
                .unwrap();
        let lineage_b_cid = lineage_b_revision
            .sign(&owner_key, Some(&ucan))
            .await
            .unwrap();

        let mut rebase_revision =
            Sphere::rebase_version(&lineage_b_cid, &lineage_a_cid, &mut store)
                .await
                .unwrap();
        let rebase_cid = rebase_revision.sign(&owner_key, Some(&ucan)).await.unwrap();

        let rebased_sphere = Sphere::at(&rebase_cid, &store);
        let rebased_links = rebased_sphere.get_content().await.unwrap();

        let parent_sphere = rebased_sphere.get_parent().await.unwrap().unwrap();
        assert_eq!(parent_sphere.cid(), &lineage_a_cid);

        let grandparent_sphere = parent_sphere.get_parent().await.unwrap().unwrap();
        assert_eq!(grandparent_sphere.cid(), &base_cid);

        assert_eq!(
            rebased_links.get(&foo_key).await.unwrap(),
            Some(&foobar_memo_link)
        );
        assert_eq!(
            rebased_links.get(&bar_key).await.unwrap(),
            Some(&baz_memo_link)
        );
        assert_eq!(
            rebased_links.get(&baz_key).await.unwrap(),
            Some(&flurb_memo_link)
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_hydrate_revisions_of_names_changes() {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (mut sphere, authorization, _) =
            Sphere::generate(&owner_did, &mut store).await.unwrap();

        let identity = IdentityIpld {
            did: owner_did.clone().into(),
            link_record: None,
        };

        let mut mutation = SphereMutation::new(&owner_did);

        mutation.identities_mut().set(&"foo".into(), &identity);

        let mut revision = sphere.apply_mutation(&mutation).await.unwrap();
        let next_cid = revision
            .sign(&owner_key, Some(&authorization))
            .await
            .unwrap();

        sphere = Sphere::at(&next_cid, &store);

        let mut mutation = SphereMutation::new(&owner_did);

        mutation.identities_mut().set(&"bar".into(), &identity);

        let mut revision = sphere.apply_mutation(&mutation).await.unwrap();
        let next_cid = revision
            .sign(&owner_key, Some(&authorization))
            .await
            .unwrap();

        sphere = Sphere::at(&next_cid, &store);

        let bundle = sphere.bundle_until_ancestor(None).await.unwrap();
        let mut other_store = MemoryStore::default();

        bundle.load_into(&mut other_store).await.unwrap();

        let timeline = Timeline::new(&other_store);
        let timeslice = timeline.slice(sphere.cid(), None);
        let items = timeslice.to_chronological().await.unwrap();

        for (cid, _) in items {
            Sphere::at(&cid, &other_store).hydrate().await.unwrap();
        }

        store.expect_replica_in(&other_store).await.unwrap();
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_hydrate_revisions_from_sparse_link_blocks() {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (mut sphere, ucan, _) = Sphere::generate(&owner_did, &mut store).await.unwrap();

        for i in 0..32u8 {
            let mut mutation = SphereMutation::new(&owner_did);
            let key = format!("key{}", i);
            let memo = MemoIpld::for_body(&mut store, &[i]).await.unwrap();

            mutation.content_mut().set(
                &key,
                &store.save::<DagCborCodec, _>(&memo).await.unwrap().into(),
            );
            let mut revision = sphere.apply_mutation(&mutation).await.unwrap();
            let next_cid = revision.sign(&owner_key, Some(&ucan)).await.unwrap();
            sphere = Sphere::at(&next_cid, &store);
        }

        let bundle = sphere.bundle_until_ancestor(None).await.unwrap();
        let mut other_store = MemoryStore::default();

        bundle.load_into(&mut other_store).await.unwrap();

        let timeline = Timeline::new(&other_store);
        let timeslice = timeline.slice(sphere.cid(), None);
        let items = timeslice.to_chronological().await.unwrap();

        for (cid, _) in items {
            Sphere::at(&cid, &other_store).hydrate().await.unwrap();
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
            Sphere::generate(&owner_did, &mut store).await.unwrap();

        let ucan = authorization
            .resolve_ucan(&UcanStore(store.clone()))
            .await
            .unwrap();

        let delegation = DelegationIpld::register("Test", &ucan.encode().unwrap(), &store)
            .await
            .unwrap();

        let mut mutation = SphereMutation::new(&owner_did);

        mutation
            .delegations_mut()
            .set(&Link::new(delegation.jwt), &delegation);

        let mut revision = sphere.apply_mutation(&mutation).await.unwrap();
        let next_cid = revision
            .sign(&owner_key, Some(&authorization))
            .await
            .unwrap();

        sphere = Sphere::at(&next_cid, &store);

        let mut mutation = SphereMutation::new(&owner_did);

        mutation.revocations_mut().set(
            &Link::new(delegation.jwt),
            &RevocationIpld::revoke(&delegation.jwt, &owner_key)
                .await
                .unwrap(),
        );

        let mut revision = sphere.apply_mutation(&mutation).await.unwrap();
        let next_cid = revision
            .sign(&owner_key, Some(&authorization))
            .await
            .unwrap();

        sphere = Sphere::at(&next_cid, &store);

        let bundle = sphere.bundle_until_ancestor(None).await.unwrap();
        let mut other_store = MemoryStore::default();

        bundle.load_into(&mut other_store).await.unwrap();

        let timeline = Timeline::new(&other_store);
        let timeslice = timeline.slice(sphere.cid(), None);
        let items = timeslice.to_chronological().await.unwrap();

        for (cid, _) in items {
            Sphere::at(&cid, &other_store).hydrate().await.unwrap();
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
            base_cid: &Link<MemoIpld>,
            author_did: &str,
            credential: &Credential,
            authorization: &Authorization,
            store: &mut Storage,
            (change_key, change_memo): (&str, &MemoIpld),
        ) -> anyhow::Result<Link<MemoIpld>> {
            let mut mutation = SphereMutation::new(author_did);
            mutation.content_mut().set(
                &change_key.into(),
                &store
                    .save::<DagCborCodec, _>(change_memo)
                    .await
                    .unwrap()
                    .into(),
            );

            let mut base_revision =
                Sphere::apply_mutation_with_cid(base_cid, &mutation, store).await?;

            base_revision.sign(credential, Some(authorization)).await
        }

        let foo_memo = MemoIpld::for_body(&mut store, b"foo").await.unwrap();
        let bar_memo = MemoIpld::for_body(&mut store, b"bar").await.unwrap();
        let baz_memo = MemoIpld::for_body(&mut store, b"baz").await.unwrap();
        let foobar_memo = MemoIpld::for_body(&mut store, b"foobar").await.unwrap();
        let flurb_memo = MemoIpld::for_body(&mut store, b"flurb").await.unwrap();

        let (sphere, authorization, _) = Sphere::generate(&owner_did, &mut store).await.unwrap();

        let base_cid = make_revision(
            sphere.cid(),
            &owner_did,
            &owner_key,
            &authorization,
            &mut store,
            ("foo", &foo_memo),
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
            ("bar", &bar_memo),
        )
        .await
        .unwrap();

        let external_cid_b = make_revision(
            &external_cid_a,
            &owner_did,
            &owner_key,
            &authorization,
            &mut external_store,
            ("foobar", &foobar_memo),
        )
        .await
        .unwrap();

        let external_bundle = Bundle::from_timeslice(
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
            ("baz", &baz_memo),
        )
        .await
        .unwrap();

        let local_cid_b = make_revision(
            &local_cid_a,
            &owner_did,
            &owner_key,
            &authorization,
            &mut store,
            ("bar", &flurb_memo),
        )
        .await
        .unwrap();

        external_bundle.load_into(&mut store).await.unwrap();

        let local_sphere = Sphere::at(&local_cid_b, &store);

        local_sphere
            .sync(&base_cid, &external_cid_b, &owner_key, Some(&authorization))
            .await
            .unwrap();
    }
}
