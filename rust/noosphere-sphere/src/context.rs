use std::sync::Arc;

use anyhow::Result;

use noosphere_api::client::Client;

use noosphere_core::{
    authority::{Access, Author, SUPPORTED_KEYS},
    data::{Did, Link, MemoIpld},
    view::{Sphere, SphereMutation},
};
use noosphere_storage::{KeyValueStore, SphereDb, Storage};
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

impl<K, S> Clone for SphereContext<K, S>
where
    K: KeyMaterial + Clone + 'static,
    S: Storage,
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

    /// Given a [Did] of a sphere, produce a [SphereContext] backed by the same credentials and
    /// storage primitives as this one, but that accesses the sphere referred to by the provided
    /// [Did].
    pub async fn traverse_by_identity(
        &self,
        _sphere_identity: &Did,
    ) -> Result<SphereContext<K, S>> {
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
            &self.db.require_version(self.identity()).await?.into(),
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

#[cfg(test)]
pub mod tests {
    use anyhow::Result;

    use noosphere_core::{data::ContentType, tracing::initialize_tracing};

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    use crate::{
        helpers::{make_valid_link_record, simulated_sphere_context, SimulationAccess},
        HasMutableSphereContext, HasSphereContext, SphereContentWrite, SpherePetnameWrite,
    };

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_validates_slug_names_when_writing() -> Result<()> {
        initialize_tracing(None);
        let valid_names: &[&str] = &["j@__/_大", "/"];
        let invalid_names: &[&str] = &[""];

        let mut sphere_context =
            simulated_sphere_context(SimulationAccess::ReadWrite, None).await?;

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
        let invalid_names: &[&str] = &[""];

        let mut sphere_context =
            simulated_sphere_context(SimulationAccess::ReadWrite, None).await?;
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
}
