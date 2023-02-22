use std::sync::Arc;

use anyhow::{anyhow, Result};
use cid::Cid;
use noosphere_api::client::Client;

use noosphere_core::{
    authority::{Access, Author, SUPPORTED_KEYS},
    data::Did,
    view::{Sphere, SphereMutation},
};
use noosphere_storage::{KeyValueStore, SphereDb, Storage};
use tokio::sync::OnceCell;
use ucan::crypto::{did::DidParser, KeyMaterial};
use url::Url;

use crate::metadata::GATEWAY_URL;

/// A [SphereContext] is an accessor construct over locally replicated sphere
/// data. It embodies both the storage layer that contains the sphere's data
/// as the information needed to verify a user's intended level of access to
/// it (e.g., local key material and [ucan::Ucan]-based authorization).
/// Additionally, the [SphereContext] maintains a reference to an API [Client]
/// that may be initialized as the network becomes available.
///
/// All interactions that pertain to a sphere, including reading or writing
/// its contents and syncing with a gateway, flow through the [SphereContext].
pub struct SphereContext<K, S>
where
    K: KeyMaterial + Clone + 'static,
    S: Storage,
{
    sphere_identity: Did,
    author: Author<K>,
    access: OnceCell<Access>,
    db: SphereDb<S>,
    did_parser: DidParser,
    client: OnceCell<Arc<Client<K, SphereDb<S>>>>,
    mutation: SphereMutation,
}

impl<K, S> SphereContext<K, S>
where
    K: KeyMaterial + Clone + 'static,
    S: Storage,
{
    pub async fn new(sphere_identity: Did, author: Author<K>, db: SphereDb<S>) -> Result<Self> {
        let author_did = author.identity().await?;

        Ok(SphereContext {
            sphere_identity,
            access: OnceCell::new(),
            author,
            db,
            did_parser: DidParser::new(SUPPORTED_KEYS),
            client: OnceCell::new(),
            mutation: SphereMutation::new(&author_did),
        })
    }

    /// Given a [Did] of a sphere, produce a [SphereContext] backed by the same credentials and
    /// storage primitives as this one, but that accesses the sphere referred to by the provided
    /// [Did].
    pub async fn traverse(&self, sphere_identity: &Did) -> Result<SphereContext<K, S>> {
        Ok(SphereContext::new(
            sphere_identity.clone(),
            self.author.clone(),
            self.db.clone(),
        )
        .await?)
    }

    /// Resolve the most recent version in the local history of the sphere
    pub async fn head(&self) -> Result<Cid> {
        let sphere_identity = self.identity();
        self.db
            .get_version(&self.sphere_identity)
            .await?
            .ok_or_else(|| anyhow!("No version found for {}", sphere_identity))
    }

    /// The identity of the sphere
    pub fn identity(&self) -> &Did {
        &self.sphere_identity
    }

    /// The [Author] who is currently accessing the sphere
    pub fn author(&self) -> &Author<K> {
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

    pub fn mutation(&self) -> &SphereMutation {
        &self.mutation
    }

    pub fn mutation_mut(&mut self) -> &mut SphereMutation {
        &mut self.mutation
    }

    /// Get a [Sphere] view over the current sphere's latest revision. This view
    /// offers lower-level access than [SphereFs], but includes affordances to
    /// help tranversing and manipulating IPLD structures that are more
    /// convenient than working directly with raw data.
    pub async fn sphere(&self) -> Result<Sphere<SphereDb<S>>> {
        Ok(Sphere::at(
            &self.db.require_version(self.identity()).await?,
            self.db(),
        ))
    }

    /// Get a [Client] that will interact with a configured gateway (if a URL
    /// for one has been configured). This will initialize a [Client] if one is
    /// not already intialized, and will fail if the [Client] is unable to
    /// verify the identity of the gateway or otherwise connect to it.
    pub async fn client(&mut self) -> Result<Arc<Client<K, SphereDb<S>>>> {
        let client = self
            .client
            .get_or_try_init::<anyhow::Error, _, _>(|| async {
                let gateway_url: Url = self.db.require_key(GATEWAY_URL).await?;

                Ok(Arc::new(
                    Client::identify(
                        &self.sphere_identity,
                        &gateway_url,
                        &self.author,
                        &mut self.did_parser,
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
    pub(crate) fn reset_access(&mut self) -> () {
        self.access.take();
    }
}
