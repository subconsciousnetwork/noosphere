use std::sync::Arc;

use anyhow::Result;
use noosphere_api::client::Client;

use noosphere_core::{
    authority::{Author, SUPPORTED_KEYS},
    data::Did,
};
use noosphere_fs::SphereFs;
use noosphere_storage::{
    db::SphereDb,
    interface::{KeyValueStore, Store},
};
use tokio::sync::OnceCell;
use ucan::crypto::{did::DidParser, KeyMaterial};
use url::Url;

use crate::error::NoosphereError;

use super::{GatewaySyncStrategy, GATEWAY_URL};

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
    pub fn new(sphere_identity: Did, author: Author<K>, db: SphereDb<S>) -> Self {
        SphereContext {
            sphere_identity,
            author,
            db,
            did_parser: DidParser::new(SUPPORTED_KEYS),
            client: OnceCell::new(),
        }
    }

    /// The identity of the sphere
    pub fn identity(&self) -> &Did {
        &self.sphere_identity
    }

    /// The [Author] who is currently accessing the sphere
    pub fn author(&self) -> &Author<K> {
        &self.author
    }

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

    /// If a gateway URL has been configured, attempt to synchronize local
    /// sphere data with the gateway. Changes on the gateway will first be
    /// fetched to local storage. Then, the local changes will be replayed on
    /// top of those changes. Finally, the synchronized local history will be
    /// pushed up to the gateway.
    pub async fn sync(&mut self) -> Result<()> {
        let sync_strategy = GatewaySyncStrategy::default();
        sync_strategy.sync(self).await?;
        Ok(())
    }
}
