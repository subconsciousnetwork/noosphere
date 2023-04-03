use std::sync::Arc;

use anyhow::{anyhow, Result};
use cid::Cid;
use futures_util::TryStreamExt;
use libipld_cbor::DagCborCodec;
use noosphere_api::client::Client;

use noosphere_core::{
    authority::{Access, Author, SUPPORTED_KEYS},
    data::{ContentType, Did, MemoIpld, SphereIpld},
    view::{Sphere, SphereMutation},
};
use noosphere_storage::{BlockStore, KeyValueStore, SphereDb, Storage};
use tokio::sync::OnceCell;
use ucan::crypto::{did::DidParser, KeyMaterial};
use url::Url;

use crate::metadata::GATEWAY_URL;

#[cfg(doc)]
use crate::has::HasSphereContext;

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
    origin_sphere_identity: Did,
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
    pub async fn new(
        sphere_identity: Did,
        author: Author<K>,
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

    /// Given a petname that has been assigned to a sphere identity within this
    /// sphere's address book, produce a [SphereContext] backed by the same
    /// credentials and storage primitives as this one, but that accesses the
    /// sphere referred to by the provided [Did]. If the local data for the
    /// sphere being traversed to is not available, an attempt will be made to
    /// replicate the data from a Noosphere Gateway.
    pub async fn traverse_by_petname(&mut self, petname: &str) -> Result<SphereContext<K, S>> {
        // Resolve petname to sphere version via address book entry

        let identity = match self
            .sphere()
            .await?
            .get_address_book()
            .await?
            .get_identities()
            .await?
            .get(&petname.to_string())
            .await?
        {
            Some(address) => address.clone(),
            None => return Err(anyhow!("\"{petname}\" is not assigned to an identity")),
        };

        let resolved_version = match identity.link_record(self.db()).await {
            Some(link_record) => link_record.dereference().await,
            None => None,
        };

        let resolved_version = match resolved_version {
            Some(cid) => cid,
            None => {
                return Err(anyhow!(
                    "No version has been resolved for \"{petname}\" ({})",
                    identity.did
                ));
            }
        };

        // Check for version in local sphere DB

        let maybe_has_resolved_version = match self.db().get_version(&identity.did).await? {
            Some(local_version) => local_version == resolved_version,
            None => false,
        };

        // If version available, check for memo and body blocks

        let should_replicate_from_gateway = if maybe_has_resolved_version {
            match self
                .db()
                .load::<DagCborCodec, MemoIpld>(&resolved_version)
                .await
            {
                Ok(memo) => {
                    if memo.content_type() != Some(ContentType::Sphere) {
                        return Err(anyhow!(
                            "Resolved content for \"{petname}\" ({}) does not refer to a sphere",
                            identity.did
                        ));
                    }

                    match self.db().load::<DagCborCodec, SphereIpld>(&memo.body).await {
                        Ok(_) => false,
                        Err(error) => {
                            warn!("{error}");
                            true
                        }
                    }
                }
                Err(error) => {
                    warn!("{error}");
                    true
                }
            }
        } else {
            true
        };

        // If no version available or memo/body missing, replicate from gateway

        if should_replicate_from_gateway {
            let client = self.client().await?;
            let stream = client.replicate(&resolved_version).await?;

            tokio::pin!(stream);

            while let Some((cid, block)) = stream.try_next().await? {
                self.db_mut().put_block(&cid, &block).await?;
            }
        }

        // Update the version in local sphere DB

        self.db_mut()
            .set_version(&identity.did, &resolved_version)
            .await?;

        // Initialize a `SphereContext` with the same author and sphere DB as
        // this one, but referring to the resolved sphere DID, and return it

        SphereContext::new(
            identity.did.clone(),
            self.author.clone(),
            self.db.clone(),
            Some(self.sphere_identity.clone()),
        )
        .await
    }

    /// Given a [Did] of a sphere, produce a [SphereContext] backed by the same credentials and
    /// storage primitives as this one, but that accesses the sphere referred to by the provided
    /// [Did].
    pub async fn traverse_by_identity(
        &self,
        _sphere_identity: &Did,
    ) -> Result<SphereContext<K, S>> {
        unimplemented!()
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
    /// offers lower-level access than [HasSphereContext], but includes affordances to
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
    pub async fn client(&self) -> Result<Arc<Client<K, SphereDb<S>>>> {
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
