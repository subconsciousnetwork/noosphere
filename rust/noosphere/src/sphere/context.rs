use std::sync::Arc;

use anyhow::Result;
use noosphere_api::client::Client;

use noosphere_core::authority::{Authorization, SUPPORTED_KEYS};
use noosphere_fs::SphereFs;
use noosphere_storage::{db::SphereDb, interface::Store};
use tokio::sync::OnceCell;
use ucan::crypto::{did::DidParser, KeyMaterial};
use url::Url;

use crate::error::NoosphereError;

use super::access::SphereAccess;

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
    K: KeyMaterial + 'static,
    S: Store,
{
    sphere_identity: String,
    gateway_url: Option<Url>,
    access: SphereAccess<Arc<K>>,
    db: SphereDb<S>,
    did_parser: DidParser,
    client: OnceCell<Arc<Client<Arc<K>, SphereDb<S>>>>,
}

impl<K, S> SphereContext<K, S>
where
    K: KeyMaterial + 'static,
    S: Store,
{
    pub fn new(
        sphere_identity: String,
        access: SphereAccess<Arc<K>>,
        db: SphereDb<S>,
        gateway_url: Option<Url>,
    ) -> Self {
        SphereContext {
            sphere_identity,
            access,
            db,
            gateway_url,
            did_parser: DidParser::new(SUPPORTED_KEYS),
            client: OnceCell::new(),
        }
    }

    /// The DID identity of the sphere
    pub fn identity(&self) -> &str {
        &self.sphere_identity
    }

    /// The key authorized to access the sphere in this context
    pub fn user_key(&self) -> Option<Arc<K>> {
        match &self.access {
            SphereAccess::ReadOnly => None,
            SphereAccess::ReadWrite { user_key, .. } => Some(user_key.clone()),
        }
    }

    /// The authorization that allows the user key to access the sphere in this context
    pub fn user_authorization(&self) -> Option<&Authorization> {
        match &self.access {
            SphereAccess::ReadOnly => None,
            SphereAccess::ReadWrite { authorization, .. } => Some(authorization),
        }
    }

    /// Get a [SphereFs] instance over the current sphere's content; note that
    /// if the user's [SphereAccess] is read-only, the returned [SphereFs] will
    /// be read-only as well.
    pub async fn fs(&self) -> Result<SphereFs<S>, NoosphereError> {
        let author_identity = match &self.access {
            SphereAccess::ReadOnly => None,
            SphereAccess::ReadWrite { user_identity, .. } => Some(user_identity.as_str()),
        };

        SphereFs::latest(&self.sphere_identity, author_identity, &self.db)
            .await
            .map_err(|e| e.into())
    }

    /// Get a [Client] that will interact with a configured gateway (if a URL
    /// for one has been configured). This will initialize a [Client] if one is
    /// not already intialized, and will fail if the [Client] is unable to
    /// verify the identity of the gateway or otherwise connect to it.
    pub async fn client(&mut self) -> Result<Arc<Client<Arc<K>, SphereDb<S>>>, NoosphereError> {
        let client = self
            .client
            .get_or_try_init(|| async {
                let gateway_url = self
                    .gateway_url
                    .clone()
                    .ok_or(NoosphereError::MissingConfiguration("gateway URL"))?;

                let (credential, authorization) = match &self.access {
                    SphereAccess::ReadOnly => return Err(NoosphereError::NoCredentials),
                    SphereAccess::ReadWrite {
                        user_key,
                        authorization,
                        ..
                    } => (user_key.clone(), authorization.clone()),
                };

                Ok(Arc::new(
                    Client::identify(
                        &self.sphere_identity,
                        &gateway_url,
                        credential,
                        &authorization,
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
