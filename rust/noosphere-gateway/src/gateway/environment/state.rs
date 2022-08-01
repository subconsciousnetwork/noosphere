use std::ops::{Deref, DerefMut};

use anyhow::Result;
use cid::Cid;
use noosphere_cbor::TryDagCbor;
use noosphere_storage::interface::{KeyValueStore, StorageProvider, Store};
use serde::{Deserialize, Serialize};

use crate::gateway::{commands::GATEWAY_STATE_STORE, schema::PublishedSphere};

#[derive(Serialize, Deserialize)]
pub struct SphereTracker {
    pub latest: Option<Cid>,
    pub published: Option<Cid>,
}

pub struct GatewayState<Storage: Store>(pub Storage);

impl<Storage: Store> GatewayState<Storage> {
    pub async fn get_or_initialize_tracker(&self, sphere: &str) -> Result<SphereTracker> {
        match self.get(sphere).await? {
            Some(sphere @ SphereTracker { .. }) => Ok(sphere),
            None => Ok(SphereTracker {
                latest: None,
                published: None,
            }),
        }
    }
}

impl<Storage: Store> GatewayState<Storage> {
    pub async fn from_storage_provider<Provider>(
        provider: &Provider,
    ) -> Result<GatewayState<Storage>>
    where
        Provider: StorageProvider<Storage>,
    {
        Ok(GatewayState(provider.get_store(GATEWAY_STATE_STORE).await?))
    }
}

impl<Storage: Store> Deref for GatewayState<Storage> {
    type Target = Storage;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<Storage: Store> DerefMut for GatewayState<Storage> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// const LATEST_CID_KEY: &str = "LATEST_CID";
// const PUBLISHED_SPHERE_KEY: &str = "PUBLISHED_SPHERE";

// pub struct GatewayState<Storage: Store> {
//     store: Storage,
// }

// impl<Storage: Store> GatewayState<Storage> {
//     pub fn new(store: Storage) -> Self {
//         GatewayState { store }
//     }

//     pub async fn get_published_sphere(&self) -> Result<Option<PublishedSphere>> {
//         Ok(
//             match self.store.read(PUBLISHED_SPHERE_KEY.as_ref()).await? {
//                 Some(bytes) => Some(PublishedSphere::try_from_dag_cbor(&bytes)?),
//                 None => None,
//             },
//         )
//     }

//     pub async fn set_published_sphere(&mut self, published_sphere: &PublishedSphere) -> Result<()> {
//         self.store
//             .write(
//                 PUBLISHED_SPHERE_KEY.as_ref(),
//                 &published_sphere.try_into_dag_cbor()?,
//             )
//             .await?;
//         Ok(())
//     }

//     pub async fn get_latest_cid(&self) -> Result<Option<Cid>> {
//         Ok(match self.store.read(LATEST_CID_KEY.as_ref()).await? {
//             Some(bytes) => Some(Cid::try_from(bytes)?),
//             None => None,
//         })
//     }

//     pub async fn set_latest_cid(&mut self, cid: &Cid) -> Result<()> {
//         self.store
//             .write(LATEST_CID_KEY.as_ref(), &cid.to_bytes())
//             .await?;
//         Ok(())
//     }
// }
