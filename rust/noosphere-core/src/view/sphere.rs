use anyhow::{anyhow, Result};
use async_stream::try_stream;
use cid::Cid;
use futures::Stream;
use libipld_cbor::DagCborCodec;
use tokio::sync::OnceCell;
use tokio_stream::StreamExt;

use ucan::{
    builder::UcanBuilder,
    chain::ProofChain,
    crypto::{did::DidParser, KeyMaterial},
    store::UcanJwtStore,
    Ucan,
};

use crate::{
    authority::{
        ed25519_key_to_mnemonic, generate_capability, generate_ed25519_key, restore_ed25519_key,
        Author, Authorization, SphereAbility, SPHERE_SEMANTICS, SUPPORTED_KEYS,
    },
    data::{
        Bundle, ChangelogIpld, ContentType, DelegationIpld, Did, Header, IdentityIpld, Link,
        MapOperation, MemoIpld, Mnemonic, RevocationIpld, SphereIpld, TryBundle, Version,
    },
    error::NoosphereError,
    view::{Content, SphereMutation, SphereRevision, Timeline},
};

use noosphere_storage::{base64_decode, block_serialize, BlockStore, SphereDb, Storage, UcanStore};

use super::{address::AddressBook, Authority, Delegations, Identities, Revocations, Timeslice};

/// An arbitrarily long value for sphere authorizations that should outlive the
/// keys they authorize
// TODO: Recent UCAN versions allow setting a null expiry; we should use that
// instead
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
    #[instrument(level = "debug", skip(self, replicate))]
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

        debug!("Petname is assigned to {:?}", identity);

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

        warn!(did = ?identity.did, "Looking for local version");

        let local_version = self
            .store()
            .get_version(&identity.did)
            .await?
            .map(|cid| cid.into());
        let (replication_required, target_version) =
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

        let target_version = if replication_required {
            debug!("Attempting to replicate from gateway...");

            if let Err(error) = replicate(link_record_version, target_version).await {
                if let Some(local_version) = local_version {
                    warn!("Replication failed; falling back to {local_version}");
                    local_version
                } else {
                    warn!(identity = ?identity.did, "Replication failed, and no fallback is available");
                    return Err(error);
                }
            } else {
                link_record_version
            }
        } else {
            link_record_version
        };

        Ok(Some(Sphere::at(&target_version, self.store())))
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
            cid: *cid,
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

    /// Get an immutable reference to the [BlockStore] that is in use by this
    /// [Sphere] view
    pub fn store(&self) -> &S {
        &self.store
    }

    /// Get a mutable reference to the [BlockStore] that is in use by this
    /// [Sphere] view
    pub fn store_mut(&mut self) -> &mut S {
        &mut self.store
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
    /// excluding the given [Link<MemoIpld>] (or until the genesis revision of the
    /// sphere if no [Link<MemoIpld>] is given).
    pub async fn bundle_until_ancestor(&self, cid: Option<&Link<MemoIpld>>) -> Result<Bundle> {
        Bundle::from_timeslice(
            &Timeline::new(&self.store)
                .slice(&self.cid, cid)
                .exclude_past(),
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

    /// Attempt to load the [AddressBook] of this sphere. If no address book is
    /// found, an empty one is initialized.
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

        for cid in items {
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
        trace!("hydrate_with_cid: to_memo");
        let memo = sphere.to_memo().await?;
        trace!("hydrate_with_cid: match");
        let base_cid = match memo.parent {
            Some(cid) => cid,
            None => {
                let base_sphere = SphereIpld::new(&sphere.get_identity().await?, store).await?;
                let empty_dag = MemoIpld::for_body(store, &base_sphere).await?;
                store.save::<DagCborCodec, _>(&empty_dag).await?.into()
            }
        };

        trace!("hydrate_with_cid: apply_mutation_with_cid");
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

        trace!("hydrate_with_cid: success");
        Ok(())
    }

    /// Compact sphere history through a given version, producing a single
    /// combined history that reflects all of the changes of the intermediate
    /// versions. The given history version must have a parent version to base
    /// the compacted history on top of. The new compacted history will be
    /// signed by the given author (authorship of the intermediate versions will
    /// be lost).
    pub async fn compact<K>(
        &self,
        until: &Link<MemoIpld>,
        author: &Author<K>,
    ) -> Result<Link<MemoIpld>>
    where
        K: KeyMaterial + Clone + 'static,
    {
        let parent_sphere =
            if let Some(parent_sphere) = Sphere::at(until, &self.store).get_parent().await? {
                parent_sphere
            } else {
                return Err(anyhow!(
                    "Cannot compact history; compound history must must have ancestral base"
                ));
            };

        let timeline = Timeline::new(&self.store);
        let timeslice = timeline.slice(&self.cid, Some(until)).include_past();

        let history_to_compact = timeslice.to_chronological().await?;
        let mut compact_mutation = SphereMutation::new(&author.did().await?);

        for link in history_to_compact {
            let mutation = Sphere::at(&link, &self.store).derive_mutation().await?;
            compact_mutation.append(mutation);
        }

        let mut revision = parent_sphere.apply_mutation(&compact_mutation).await?;

        revision
            .sign(&author.key, author.authorization.as_ref())
            .await
    }

    /// Attempt to linearize the canonical history of the sphere by re-basing
    /// the history onto a branch with an implicitly common lineage.
    pub async fn rebase<Credential: KeyMaterial>(
        &self,
        old_base: &Link<MemoIpld>,
        new_base: &Link<MemoIpld>,
        credential: &Credential,
        authorization: Option<&Authorization>,
    ) -> Result<Link<MemoIpld>> {
        let mut store = self.store.clone();

        let timeline = Timeline::new(&self.store);
        Sphere::hydrate_timeslice(&timeline.slice(new_base, Some(old_base)).exclude_past()).await?;

        let timeline = Timeline::new(&self.store);
        let timeslice = timeline.slice(self.cid(), Some(old_base)).exclude_past();
        let rebase_revisions = timeslice.to_chronological().await?;

        let mut next_base = *new_base;

        for cid in rebase_revisions.iter() {
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
    ) -> Result<(Sphere<S>, Authorization, Mnemonic)> {
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

        let capability = generate_capability(&sphere_did, SphereAbility::Authorize);
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
    #[deprecated(note = "Use SphereAuthorityWrite::recover_authority instead")]
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

        let authorize_capability = generate_capability(&sphere_did, SphereAbility::Authorize);
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
            let timeslice = timeline.slice(&self.cid, since.as_ref()).exclude_past();
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

    /// Verify that a given authorization is a valid with regards to operating
    /// on this [Sphere]; it is issued by the sphere (or appropriately
    /// delegated), and it has not been revoked.
    pub async fn verify_authorization(
        &self,
        authorization: &Authorization,
    ) -> Result<(), NoosphereError> {
        let proof_chain = authorization
            .as_proof_chain(&UcanStore(self.store.clone()))
            .await?;

        let authority = self.get_authority().await?;
        let delegations = authority.get_delegations().await?;
        let revocations = authority.get_revocations().await?;
        let sphere_identity = self.get_identity().await?;
        let sphere_credential = sphere_identity.to_credential()?;

        let mut remaining_links = vec![&proof_chain];

        while let Some(chain_link) = remaining_links.pop() {
            let link = Link::from(chain_link.ucan().to_cid(cid::multihash::Code::Blake3_256)?);

            if delegations.get(&link).await?.is_none() {
                return Err(NoosphereError::InvalidAuthorization(
                    authorization.clone(),
                    "Not found in sphere authority".into(),
                ));
            }

            let ucan_issuer = chain_link.ucan().issuer();

            if let Some(revocation) = revocations.get(&link).await? {
                // NOTE: The implication here is that only the sphere itself or
                // the direct issuer may issue revocations
                if revocation.iss != sphere_identity || revocation.iss != ucan_issuer {
                    warn!("Revocation for {} had an invalid issuer; expected {} or {}, but found {}; skipping...", link, ucan_issuer, sphere_identity, revocation.iss);
                    continue;
                }

                if let Err(error) = revocation.verify(&sphere_credential).await {
                    warn!("Unverifiable revocation: {}", error);
                    continue;
                }

                return Err(NoosphereError::InvalidAuthorization(
                    authorization.clone(),
                    format!("Revoked by {}", link),
                ));
            }

            for proof in chain_link.proofs() {
                remaining_links.push(proof);
            }
        }

        Ok(())
    }

    /// Validate this sphere revision's signature and proof chain
    // TODO(#421): Allow this to be done at a specific "now" time, to cover the
    // case when we are verifying a historical revision with a possibly expired
    // credential
    //
    // TODO(#422): This also needs to take revocations into account, probably by
    // calling to `Sphere::verify_authorization`
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
            let now_time = if let Some(nbf) = ucan.not_before() {
                Some(nbf.to_owned())
            } else {
                ucan.expires_at().as_ref().map(|exp| exp - 1)
            };

            let ucan_store = UcanStore(self.store.clone());
            let proof = ProofChain::from_ucan(ucan, now_time, &mut did_parser, &ucan_store).await?;

            let desired_capability =
                generate_capability(&sphere_ipld.identity, SphereAbility::Push);

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
            let mut parent = None;

            for item in history.into_iter().rev() {
                let (cid, sphere) = item?;
                let parent_sphere = match parent {
                    Some(_) => parent,
                    None => sphere.get_parent().await?
                };

                let identities = sphere.get_address_book().await?.get_identities().await?;
                let may_yield_changelog = match parent_sphere {
                    Some(parent_sphere) => identities.cid() != parent_sphere.get_address_book().await?.get_identities().await?.cid(),
                    None => true
                };

                if may_yield_changelog {
                    let changelog = identities.load_changelog().await?;

                    if !changelog.is_empty() {
                        yield (cid, changelog);
                    }
                }

                parent = Some(sphere);
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
            let mut parent = None;

            for item in history.into_iter().rev() {
                let (cid, sphere) = item?;
                let parent_sphere = match parent {
                    Some(_) => parent,
                    None => sphere.get_parent().await?
                };

                let content = sphere.get_content().await?;
                let may_yield_changelog = match parent_sphere {
                    Some(parent_sphere) => content.cid() != parent_sphere.get_content().await?.cid(),
                    None => true
                };

                if may_yield_changelog {
                    let changelog = content.load_changelog().await?;

                    if !changelog.is_empty() {
                        yield (cid, changelog);
                    }
                }

                parent = Some(sphere);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use cid::Cid;
    use libipld_cbor::DagCborCodec;
    use tokio_stream::StreamExt;
    use ucan::{builder::UcanBuilder, crypto::KeyMaterial};

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use super::*;
    use crate::{
        authority::{generate_ed25519_key, Authorization, SphereAbility},
        data::{Bundle, DelegationIpld, IdentityIpld, Link, MemoIpld, RevocationIpld},
        helpers::make_valid_link_record,
        tracing::initialize_tracing,
        view::{Sphere, SphereMutation, Timeline, SPHERE_LIFETIME},
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

            (*sphere.cid(), sphere.get_identity().await.unwrap())
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
        let mut lineage = vec![*sphere.cid()];
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
        let mut lineage = vec![*sphere.cid()];

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

        let since = lineage[2];

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
    async fn it_can_verify_an_authorization_to_write_to_a_sphere() -> Result<()> {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await?;

        let (sphere, authorization, _) = Sphere::generate(&owner_did, &mut store).await?;

        sphere.verify_authorization(&authorization).await?;

        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_wont_verify_an_invalid_authorization_to_write_to_a_sphere() -> Result<()> {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await?;

        let other_key = generate_ed25519_key();
        let other_did = other_key.get_did().await?;

        let (sphere, _, _) = Sphere::generate(&owner_did, &mut store).await?;

        let invalid_authorization = Authorization::Ucan(
            UcanBuilder::default()
                .issued_by(&owner_key)
                .for_audience(&other_did)
                .with_lifetime(SPHERE_LIFETIME)
                .claiming_capability(&generate_capability(&other_did, SphereAbility::Publish))
                .build()?
                .sign()
                .await?,
        );

        assert!(
            sphere
                .verify_authorization(&invalid_authorization)
                .await
                .is_err(),
            "Authorization is invalid (authorizor has no authority over resource)"
        );

        Ok(())
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

        for cid in items {
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

        for cid in items {
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
            .as_ucan(&UcanStore(store.clone()))
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

        for cid in items {
            Sphere::at(&cid, &other_store).hydrate().await.unwrap();
        }

        store.expect_replica_in(&other_store).await.unwrap();
    }

    async fn make_content_revision<Credential: KeyMaterial, Storage: Store>(
        base_cid: &Link<MemoIpld>,
        author_did: &str,
        credential: &Credential,
        authorization: &Authorization,
        store: &mut Storage,
        (change_key, change_memo): (&str, &MemoIpld),
    ) -> Result<Link<MemoIpld>> {
        let mut mutation = SphereMutation::new(author_did);
        mutation.content_mut().set(
            &change_key.into(),
            &store.save::<DagCborCodec, _>(change_memo).await?.into(),
        );

        let mut base_revision = Sphere::apply_mutation_with_cid(base_cid, &mutation, store).await?;

        base_revision.sign(credential, Some(authorization)).await
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_sync_a_lineage_with_external_changes() {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let foo_memo = MemoIpld::for_body(&mut store, b"foo").await.unwrap();
        let bar_memo = MemoIpld::for_body(&mut store, b"bar").await.unwrap();
        let baz_memo = MemoIpld::for_body(&mut store, b"baz").await.unwrap();
        let foobar_memo = MemoIpld::for_body(&mut store, b"foobar").await.unwrap();
        let flurb_memo = MemoIpld::for_body(&mut store, b"flurb").await.unwrap();

        let (sphere, authorization, _) = Sphere::generate(&owner_did, &mut store).await.unwrap();

        let base_cid = make_content_revision(
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

        let external_cid_a = make_content_revision(
            &base_cid,
            &owner_did,
            &owner_key,
            &authorization,
            &mut external_store,
            ("bar", &bar_memo),
        )
        .await
        .unwrap();

        let external_cid_b = make_content_revision(
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

        let local_cid_a = make_content_revision(
            &base_cid,
            &owner_did,
            &owner_key,
            &authorization,
            &mut store,
            ("baz", &baz_memo),
        )
        .await
        .unwrap();

        let local_cid_b = make_content_revision(
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
            .rebase(&base_cid, &external_cid_b, &owner_key, Some(&authorization))
            .await
            .unwrap();
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_compacts_basic_history_into_a_single_revision() -> Result<()> {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await?;

        let (sphere, authorization, _) = Sphere::generate(&owner_did, &mut store).await?;

        let base = *sphere.cid();
        let mut long_history_version = base;

        for content in ["foo", "bar", "baz"] {
            let memo = MemoIpld::for_body(&mut store, content.as_bytes()).await?;
            long_history_version = make_content_revision(
                &long_history_version,
                &owner_did,
                &owner_key,
                &authorization,
                &mut store,
                (content, &memo),
            )
            .await?;
        }

        let long_history_sphere = Sphere::at(&long_history_version, &store);
        let mut until_sphere = long_history_sphere.clone();

        for _ in 0..2 {
            until_sphere = until_sphere.get_parent().await?.unwrap();
        }

        let author = Author {
            key: owner_key,
            authorization: Some(authorization),
        };

        let compacted_history_version = long_history_sphere
            .compact(until_sphere.cid(), &author)
            .await?;

        let compacted_history_sphere = Sphere::at(&compacted_history_version, &store);

        let mutation = compacted_history_sphere.derive_mutation().await?;

        assert_eq!(
            mutation
                .content()
                .changes()
                .iter()
                .map(|op| match op {
                    MapOperation::Add { key, .. } => key.as_str(),
                    MapOperation::Remove { key } => key.as_str(),
                })
                .collect::<Vec<&str>>(),
            vec!["foo", "bar", "baz"]
        );

        assert_eq!(
            compacted_history_sphere.get_parent().await?.unwrap().cid(),
            &base
        );

        for content in ["foo", "bar", "baz"] {
            let compacted_content = compacted_history_sphere.get_content().await?;
            let long_history_content = long_history_sphere.get_content().await?;
            assert_eq!(
                compacted_content
                    .get_as_cid::<DagCborCodec>(&content.into())
                    .await?,
                long_history_content
                    .get_as_cid::<DagCborCodec>(&content.into())
                    .await?
            );
        }

        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_compacts_nontrivial_history_into_a_single_revision() -> Result<()> {
        initialize_tracing(None);

        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await?;

        let (sphere, authorization, _) = Sphere::generate(&owner_did, &mut store).await?;

        let base = *sphere.cid();
        let mut long_history_version = base;

        for i in 0..100 {
            let petname_id = (i / 6 - 1) * 6;
            let petname_change = i % 6 == 0;
            let additive_change = i == 0 || i % 15 != 0;

            let mut mutation = SphereMutation::new(&owner_did);

            let memo = MemoIpld::for_body(&mut store, format!("content{i}")).await?;
            let link = store.save::<DagCborCodec, _>(memo).await?;

            mutation
                .content_mut()
                .set(&format!("slug{i}"), &link.into());

            if additive_change {
                mutation
                    .content_mut()
                    .set(&format!("slug{i}"), &link.into());
            } else {
                mutation.content_mut().remove(&format!("slug{}", i - 1));
            }

            if petname_change {
                if additive_change {
                    let (did, _, link) =
                        make_valid_link_record(&mut UcanStore(store.clone())).await?;
                    let identity = IdentityIpld {
                        did,
                        link_record: Some(link),
                    };
                    mutation
                        .identities_mut()
                        .set(&format!("petname{i}"), &identity);
                } else {
                    mutation
                        .identities_mut()
                        .remove(&format!("petname{}", petname_id));
                }
            }

            let mut revision = Sphere::at(&long_history_version, &store)
                .apply_mutation(&mutation)
                .await?;

            long_history_version = revision.sign(&owner_key, Some(&authorization)).await?;
        }

        let long_history_sphere = Sphere::at(&long_history_version, &store);
        let mut until_sphere = long_history_sphere.clone();

        for _ in 0..99 {
            until_sphere = until_sphere.get_parent().await?.unwrap();
        }

        let author = Author {
            key: owner_key,
            authorization: Some(authorization),
        };

        let compacted_history_version = long_history_sphere
            .compact(until_sphere.cid(), &author)
            .await?;

        let compacted_history_sphere = Sphere::at(&compacted_history_version, &store);

        let mutation = compacted_history_sphere.derive_mutation().await?;

        assert_eq!(mutation.identities().changes().len(), 14);
        assert_eq!(mutation.content().changes().len(), 100);

        let content = compacted_history_sphere.get_content().await?;
        let identities = compacted_history_sphere
            .get_address_book()
            .await?
            .get_identities()
            .await?;

        debug!("{:#?}", mutation.identities().changes());

        for i in 0..99 {
            let petname_change = i % 6 == 0;
            let removed_petname_change = (i + 6) % 15 == 0;
            let removed_content_change = (i + 1) % 15 == 0;
            let added_change = i == 0 || i % 15 != 0;
            let added_content_change = !removed_content_change && added_change;
            let added_petname_change = !removed_petname_change && added_change;

            debug!("i: {i}, added content: {added_content_change}, removed content: {removed_content_change}, added petname: {added_petname_change}, removed petname: {removed_petname_change}");

            if added_content_change {
                assert!(content
                    .get_as_cid::<DagCborCodec>(&format!("slug{}", i))
                    .await?
                    .is_some());
            } else if removed_content_change {
                assert!(content
                    .get_as_cid::<DagCborCodec>(&format!("slug{}", i))
                    .await?
                    .is_none());
            }

            if petname_change {
                if added_petname_change {
                    assert!(identities
                        .get_as_cid::<DagCborCodec>(&format!("petname{}", i))
                        .await?
                        .is_some());
                } else if removed_petname_change {
                    assert!(identities
                        .get_as_cid::<DagCborCodec>(&format!("petname{}", i))
                        .await?
                        .is_none());
                }
            }
        }

        Ok(())
    }
}
