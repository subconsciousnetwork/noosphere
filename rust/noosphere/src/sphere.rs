use std::{collections::BTreeMap, sync::Arc};

use anyhow::{anyhow, Result};
use noosphere_api::{
    client::Client,
    data::{FetchParameters, FetchResponse},
};
use noosphere_core::{authority::Authorization, view::Sphere};
use noosphere_fs::SphereFs;
use noosphere_storage::{db::SphereDb, interface::Store};
use ucan::crypto::KeyMaterial;

use crate::error::NoosphereError;

pub enum SphereAccess<K> {
    ReadOnly,
    ReadWrite {
        user_key: K,
        user_identity: String,
        authorization: Authorization,
    },
}

pub enum SphereNetwork<'a, K, S>
where
    K: KeyMaterial,
    S: Store,
{
    Online {
        client: Arc<Client<'a, K, SphereDb<S>>>,
    },
    Offline,
}

pub struct SphereContext<'a, K, S>
where
    K: KeyMaterial,
    S: Store,
{
    user_identity: String,
    sphere_identity: String,
    access: SphereAccess<K>,
    db: SphereDb<S>,
    network: SphereNetwork<'a, K, S>,
}

impl<'a, K, S> SphereContext<'a, K, S>
where
    K: KeyMaterial,
    S: Store,
{
    pub async fn fs(&self) -> Result<SphereFs<S>, NoosphereError> {
        let author_identity = match &self.access {
            SphereAccess::ReadOnly => None,
            SphereAccess::ReadWrite { user_identity, .. } => Some(user_identity.as_str()),
        };

        SphereFs::latest(&self.sphere_identity, author_identity, &self.db)
            .await
            .map_err(|e| e.into())
    }

    fn require_online(&self) -> Result<Arc<Client<'a, K, SphereDb<S>>>, NoosphereError> {
        match &self.network {
            SphereNetwork::Online { client } => Ok(client.clone()),
            SphereNetwork::Offline => Err(NoosphereError::NetworkOffline),
        }
    }
}
