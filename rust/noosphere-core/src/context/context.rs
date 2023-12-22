use std::sync::Arc;

use anyhow::Result;

use crate::{
    api::Client,
    authority::{Access, Author, SUPPORTED_KEYS},
    context::metadata::GATEWAY_URL,
    data::{Did, Link, MemoIpld},
    view::{Sphere, SphereMutation},
};
use noosphere_storage::{KeyValueStore, SphereDb, Storage};
use tokio::sync::OnceCell;
use ucan::crypto::{did::DidParser, KeyMaterial};
use url::Url;

#[cfg(doc)]
use crate::context::has::HasSphereContext;

/// The type of any [KeyMaterial] that is used within a [SphereContext]
pub type SphereContextKey = Arc<Box<dyn KeyMaterial>>;

/// A [SphereContext] is an accessor construct over locally replicated sphere
/// data. It embodies both the storage layer that contains the sphere's data
/// as the information needed to verify a user's intended level of access to
/// it (e.g., local key material and [ucan::Ucan]-based authorization).
/// Additionally, the [SphereContext] maintains a reference to an API [Client]
/// that may be initialized as the network becomes available.
///
/// All interactions that pertain to a sphere, including reading or writing
/// its contents and syncing with a gateway, flow through the [SphereContext].
pub struct SphereContext<S>
where
    S: Storage + 'static,
{
    sphere_identity: Did,
    origin_sphere_identity: Did,
    author: Author<SphereContextKey>,
    access: OnceCell<Access>,
    db: SphereDb<S>,
    did_parser: DidParser,
    client: OnceCell<Arc<Client<SphereContextKey, SphereDb<S>>>>,
    mutation: SphereMutation,
}

impl<S> Clone for SphereContext<S>
where
    S: Storage + 'static,
{
    fn clone(&self) -> Self {
        Self {
            sphere_identity: self.sphere_identity.clone(),
            origin_sphere_identity: self.origin_sphere_identity.clone(),
            author: self.author.clone(),
            access: OnceCell::new(),
            db: self.db.clone(),
            did_parser: DidParser::new(SUPPORTED_KEYS),
            client: self.client.clone(),
            mutation: SphereMutation::new(self.mutation.author()),
        }
    }
}

impl<S> SphereContext<S>
where
    S: Storage,
{
    /// Instantiate a new [SphereContext] given a sphere [Did], an [Author], a
    /// [SphereDb] and an optional origin sphere [Did]. The origin sphere [Did]
    /// is intended to signify whether the [SphereContext] is a local sphere, or
    /// a global sphere that is being visited by a local author. In most cases,
    /// a [SphereContext] with _some_ value set as the origin sphere [Did] will
    /// be read-only.
    pub async fn new(
        sphere_identity: Did,
        author: Author<SphereContextKey>,
        db: SphereDb<S>,
        origin_sphere_identity: Option<Did>,
    ) -> Result<Self> {
        let author_did = author.identity().await?;
        let origin_sphere_identity =
            origin_sphere_identity.unwrap_or_else(|| sphere_identity.clone());

        Ok(SphereContext {
            sphere_identity,
            origin_sphere_identity,
            access: OnceCell::new(),
            author,
            db,
            did_parser: DidParser::new(SUPPORTED_KEYS),
            client: OnceCell::new(),
            mutation: SphereMutation::new(&author_did),
        })
    }

    /// Clone this [SphereContext], setting the sphere identity to a peer's [Did]
    pub async fn to_visitor(&self, peer_identity: &Did) -> Result<Self> {
        self.db().require_version(peer_identity).await?;

        SphereContext::new(
            peer_identity.clone(),
            self.author.clone(),
            self.db.clone(),
            Some(self.origin_sphere_identity.clone()),
        )
        .await
    }

    /// Clone this [SphereContext], replacing the [Author] with the provided one
    pub async fn with_author(&self, author: &Author<SphereContextKey>) -> Result<SphereContext<S>> {
        SphereContext::new(
            self.sphere_identity.clone(),
            author.clone(),
            self.db.clone(),
            Some(self.origin_sphere_identity.clone()),
        )
        .await
    }

    /// Given a [Did] of a sphere, produce a [SphereContext] backed by the same credentials and
    /// storage primitives as this one, but that accesses the sphere referred to by the provided
    /// [Did].
    pub async fn traverse_by_identity(&self, _sphere_identity: &Did) -> Result<SphereContext<S>> {
        unimplemented!()
    }

    /// The identity of the sphere
    pub fn identity(&self) -> &Did {
        &self.sphere_identity
    }

    /// The identity of the gateway sphere in use during this session, if
    /// any; note that this will cause a request to be made to a gateway if no
    /// handshake has yet occurred.
    pub async fn gateway_identity(&self) -> Result<Did> {
        Ok(self.client().await?.session.gateway_identity.clone())
    }

    /// The CID of the most recent local version of this sphere
    pub async fn version(&self) -> Result<Link<MemoIpld>> {
        Ok(self.db().require_version(self.identity()).await?.into())
    }

    /// The [Author] who is currently accessing the sphere
    pub fn author(&self) -> &Author<SphereContextKey> {
        &self.author
    }

    /// The [Access] level that the configured [Author] has relative to the
    /// sphere that this [SphereContext] refers to.
    pub async fn access(&self) -> Result<Access> {
        let access = self
            .access
            .get_or_try_init(|| async {
                self.author.access_to(&self.sphere_identity, &self.db).await
            })
            .await?;
        Ok(access.clone())
    }

    /// Get a mutable reference to the [DidParser] used in this [SphereContext]
    pub fn did_parser_mut(&mut self) -> &mut DidParser {
        &mut self.did_parser
    }

    /// Sets or unsets the gateway URL that points to the gateway API that the
    /// sphere will use when it is syncing.
    pub async fn configure_gateway_url(&mut self, url: Option<&Url>) -> Result<()> {
        self.client = OnceCell::new();

        match url {
            Some(url) => {
                self.db.set_key(GATEWAY_URL, url.to_string()).await?;
            }
            None => {
                self.db.unset_key(GATEWAY_URL).await?;
            }
        }

        Ok(())
    }

    /// Get the [SphereDb] instance that manages the current sphere's block
    /// space and persisted configuration.
    pub fn db(&self) -> &SphereDb<S> {
        &self.db
    }

    /// Get a mutable reference to the [SphereDb] instance that manages the
    /// current sphere's block space and persisted configuration.
    pub fn db_mut(&mut self) -> &mut SphereDb<S> {
        &mut self.db
    }

    /// Get a read-only reference to the underlying [SphereMutation] that this
    /// [SphereContext] is tracking
    pub fn mutation(&self) -> &SphereMutation {
        &self.mutation
    }

    /// Get a mutable reference to the underlying [SphereMutation] that this
    /// [SphereContext] is tracking
    pub fn mutation_mut(&mut self) -> &mut SphereMutation {
        &mut self.mutation
    }

    /// Get a [Sphere] view over the current sphere's latest revision. This view
    /// offers lower-level access than [HasSphereContext], but includes affordances to
    /// help tranversing and manipulating IPLD structures that are more
    /// convenient than working directly with raw data.
    pub async fn sphere(&self) -> Result<Sphere<SphereDb<S>>> {
        Ok(Sphere::at(
            &self.db.require_version(self.identity()).await?.into(),
            self.db(),
        ))
    }

    /// Get a [Client] that will interact with a configured gateway (if a URL
    /// for one has been configured). This will initialize a [Client] if one is
    /// not already intialized, and will fail if the [Client] is unable to
    /// verify the identity of the gateway or otherwise connect to it.
    pub async fn client(&self) -> Result<Arc<Client<SphereContextKey, SphereDb<S>>>> {
        let client = self
            .client
            .get_or_try_init::<anyhow::Error, _, _>(|| async {
                let gateway_url: Url = self.db.require_key(GATEWAY_URL).await?;

                Ok(Arc::new(
                    Client::identify(
                        &self.origin_sphere_identity,
                        &gateway_url,
                        &self.author,
                        // TODO: Kill `DidParser` with fire
                        &mut DidParser::new(SUPPORTED_KEYS),
                        self.db.clone(),
                    )
                    .await?,
                ))
            })
            .await?;

        Ok(client.clone())
    }

    // Reset access so that it is re-evaluated the next time it is measured
    // self.access.take();
    pub(crate) fn reset_access(&mut self) {
        self.access.take();
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use crate::{
        authority::{generate_capability, generate_ed25519_key, Access, SphereAbility},
        context::{
            HasMutableSphereContext, HasSphereContext, SphereContentWrite, SpherePetnameWrite,
        },
        data::{ContentType, LinkRecord, LINK_RECORD_FACT_NAME},
        helpers::{make_valid_link_record, simulated_sphere_context},
        tracing::initialize_tracing,
        view::Sphere,
    };

    use noosphere_storage::{MemoryStorage, SphereDb};
    use ucan::{builder::UcanBuilder, crypto::KeyMaterial, store::UcanJwtStore};
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_validates_slug_names_when_writing() -> Result<()> {
        initialize_tracing(None);
        let valid_names: &[&str] = &["j@__/_大", "/"];
        let invalid_names: &[&str] = &[""];

        let (mut sphere_context, _) = simulated_sphere_context(Access::ReadWrite, None).await?;

        for invalid_name in invalid_names {
            assert!(sphere_context
                .write(invalid_name, &ContentType::Text, "hello".as_ref(), None,)
                .await
                .is_err());
        }

        for valid_name in valid_names {
            assert!(sphere_context
                .write(valid_name, &ContentType::Text, "hello".as_ref(), None,)
                .await
                .is_ok());
        }

        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_validates_petnames_when_setting() -> Result<()> {
        initialize_tracing(None);
        let valid_names: &[&str] = &["j@__/_大"];
        let invalid_names: &[&str] = &["", "did:key:foo"];

        let (mut sphere_context, _) = simulated_sphere_context(Access::ReadWrite, None).await?;
        let mut db = sphere_context.sphere_context().await?.db().clone();
        let (other_identity, link_record, _) = make_valid_link_record(&mut db).await?;

        for invalid_name in invalid_names {
            assert!(sphere_context
                .set_petname_record(invalid_name, &link_record)
                .await
                .is_err());
            assert!(sphere_context
                .set_petname(invalid_name, Some(other_identity.clone()))
                .await
                .is_err());
        }

        for valid_name in valid_names {
            assert!(sphere_context
                .set_petname(valid_name, Some(other_identity.clone()))
                .await
                .is_ok());
            sphere_context.save(None).await?;
            assert!(sphere_context
                .set_petname_record(valid_name, &link_record)
                .await
                .is_ok());
        }

        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_disallows_adding_self_as_petname() -> Result<()> {
        initialize_tracing(None);

        let (mut sphere_context, _) = simulated_sphere_context(Access::ReadWrite, None).await?;
        let db = sphere_context.sphere_context().await?.db().clone();
        let sphere_identity = sphere_context.identity().await?;

        let link_record = {
            let version = sphere_context.version().await?;
            let author = sphere_context.sphere_context().await?.author().clone();
            LinkRecord::from(
                UcanBuilder::default()
                    .issued_by(&author.key)
                    .for_audience(&sphere_identity)
                    .witnessed_by(
                        &author.authorization.as_ref().unwrap().as_ucan(&db).await?,
                        None,
                    )
                    .claiming_capability(&generate_capability(
                        &sphere_identity,
                        SphereAbility::Publish,
                    ))
                    .with_lifetime(120)
                    .with_fact(LINK_RECORD_FACT_NAME, version.to_string())
                    .build()?
                    .sign()
                    .await?,
            )
        };

        assert!(sphere_context
            .set_petname_record("myself", &link_record)
            .await
            .is_err());
        assert!(sphere_context
            .set_petname("myself", Some(sphere_identity.clone()))
            .await
            .is_err());

        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_disallows_adding_outdated_records() -> Result<()> {
        initialize_tracing(None);

        let (mut sphere_context, _) = simulated_sphere_context(Access::ReadWrite, None).await?;
        let mut store = sphere_context.sphere_context().await?.db().clone();

        // Generate two LinkRecords, the first one having a later expiry
        // than the second.
        let (records, foo_identity) = {
            let mut records: Vec<LinkRecord> = vec![];
            let owner_key = generate_ed25519_key();
            let owner_did = owner_key.get_did().await?;
            let mut db = SphereDb::new(MemoryStorage::default()).await?;
            let (sphere, proof, _) = Sphere::generate(&owner_did, &mut db).await?;
            let ucan_proof = proof.as_ucan(&db).await?;
            let sphere_identity = sphere.get_identity().await?;
            store.write_token(&ucan_proof.encode()?).await?;

            for lifetime in [500, 100] {
                let link_record = LinkRecord::from(
                    UcanBuilder::default()
                        .issued_by(&owner_key)
                        .for_audience(&sphere_identity)
                        .witnessed_by(&ucan_proof, None)
                        .claiming_capability(&generate_capability(
                            &sphere_identity,
                            SphereAbility::Publish,
                        ))
                        .with_lifetime(lifetime)
                        .with_fact(LINK_RECORD_FACT_NAME, sphere.cid().to_string())
                        .build()?
                        .sign()
                        .await?,
                );

                store.write_token(&link_record.encode()?).await?;
                records.push(link_record);
            }
            (records, sphere_identity)
        };

        sphere_context
            .set_petname("foo", Some(foo_identity))
            .await?;
        sphere_context.save(None).await?;

        assert!(sphere_context
            .set_petname_record("foo", records.get(0).unwrap())
            .await
            .is_ok());
        sphere_context.save(None).await?;
        assert!(sphere_context
            .set_petname_record("foo", records.get(1).unwrap())
            .await
            .is_err());
        Ok(())
    }
}
