use std::sync::Arc;

use anyhow::Result;
use noosphere_api::client::Client;

use noosphere_core::{
    authority::{Author, SUPPORTED_KEYS},
    data::Did,
};
use noosphere_fs::SphereFs;
use noosphere_storage::{db::SphereDb, interface::Store};
use tokio::sync::OnceCell;
use ucan::crypto::{did::DidParser, KeyMaterial};
use url::Url;

use crate::error::NoosphereError;

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
    S: Store,
{
    sphere_identity: Did,
    gateway_url: Option<Url>,
    author: Author<K>,
    db: SphereDb<S>,
    did_parser: DidParser,
    client: OnceCell<Arc<Client<K, SphereDb<S>>>>,
}

impl<K, S> SphereContext<K, S>
where
    K: KeyMaterial + Clone + 'static,
    S: Store,
{
    pub fn new(
        sphere_identity: Did,
        author: Author<K>,
        db: SphereDb<S>,
        gateway_url: Option<Url>,
    ) -> Self {
        SphereContext {
            sphere_identity,
            author,
            db,
            gateway_url,
            did_parser: DidParser::new(SUPPORTED_KEYS),
            client: OnceCell::new(),
        }
    }

    /// The identity of the sphere
    pub fn identity(&self) -> &Did {
        &self.sphere_identity
    }

    /// Get a [SphereFs] instance over the current sphere's content; note that
    /// if the user's [SphereAccess] is read-only, the returned [SphereFs] will
    /// be read-only as well.
    pub async fn fs(&self) -> Result<SphereFs<S, K>, NoosphereError> {
        SphereFs::latest(&self.sphere_identity, &self.author, &self.db)
            .await
            .map_err(|e| e.into())
    }

    /// Get a [Client] that will interact with a configured gateway (if a URL
    /// for one has been configured). This will initialize a [Client] if one is
    /// not already intialized, and will fail if the [Client] is unable to
    /// verify the identity of the gateway or otherwise connect to it.
    pub async fn client(&mut self) -> Result<Arc<Client<K, SphereDb<S>>>, NoosphereError> {
        let client = self
            .client
            .get_or_try_init::<anyhow::Error, _, _>(|| async {
                let gateway_url = self
                    .gateway_url
                    .clone()
                    .ok_or(NoosphereError::MissingConfiguration("gateway URL"))?;

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
}
