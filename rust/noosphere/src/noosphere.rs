//! A high-level, batteries-included API for Noosphere embedders

use anyhow::{anyhow, Result};
use noosphere_core::{authority::Authorization, data::Did};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use noosphere_core::context::{SphereContext, SphereCursor};
use tokio::sync::Mutex;
use url::Url;

use crate::{
    key::KeyStorage,
    platform::{PlatformKeyStorage, PlatformSphereChannel},
    sphere::{SphereChannel, SphereContextBuilder, SphereReceipt},
};

/// An enum describing different storage stragies that may be interesting
/// depending on the environment and implementation of Noosphere
#[derive(Clone)]
pub enum NoosphereStorage {
    /// Scoped storage implies that the given path is a root and that spheres
    /// should be stored in a sub-path that includes the sphere identity at the
    /// trailing end
    Scoped { path: PathBuf },

    /// Unscoped storage implies that sphere data should be kept at the given
    /// path. Note that this is typically only appropriate when dealing with a
    /// single sphere.
    Unscoped { path: PathBuf },
}

/// This enum exists so that we can incrementally layer on support for secure
/// key storage over time. Each member represents a set of environmental
/// qualities, with the most basic represnting an environment with no trusted
/// hardware key storage.
#[derive(Clone)]
pub enum NoosphereSecurity {
    /// Insecure configuration should be used on a platform where no TPM or
    /// similar secure key storage is available.
    Insecure { path: PathBuf },

    /// Opaque security configuration may be used in the case where there is
    /// some kind of protected keyring-like API layer where secret key material
    /// may be considered safely stored. For example: secret service on Linux
    /// or the keyring on MacOS.
    Opaque,
}

/// An enum describing the possible network configurations that are able to be
/// used by the Noosphere implementation
#[derive(Clone)]
pub enum NoosphereNetwork {
    /// Uses an HTTP REST client to interact with various network resources
    Http {
        gateway_api: Option<Url>,
        ipfs_gateway_url: Option<Url>,
    },
}

/// Configuration needed in order to initialize a [NoosphereContext]. This
/// configuration is intended to be flexible enough to adapt to both target
/// platform and use case of the implementing application.
#[derive(Clone)]
pub struct NoosphereContextConfiguration {
    pub storage: NoosphereStorage,
    pub security: NoosphereSecurity,
    pub network: NoosphereNetwork,
}

/// A [NoosphereContext] holds configuration necessary to initialize and store
/// Noosphere data. It also keeps a running list of active [SphereContext]
/// instances to avoid the expensive action of repeatedly opening and closing
/// a handle to backing storage for spheres that are being accessed regularly.
pub struct NoosphereContext {
    configuration: NoosphereContextConfiguration,
    sphere_channels: Arc<Mutex<BTreeMap<Did, PlatformSphereChannel>>>,
}

impl NoosphereContext {
    /// Initialize a [NoosphereContext] with a [NoosphereContextConfiguration]
    pub fn new(configuration: NoosphereContextConfiguration) -> Result<Self> {
        Ok(NoosphereContext {
            configuration,
            sphere_channels: Default::default(),
        })
    }

    async fn key_storage(&self) -> Result<PlatformKeyStorage> {
        #[cfg(target_arch = "wasm32")]
        {
            match &self.configuration.security {
                NoosphereSecurity::Opaque { .. } => PlatformKeyStorage::new("noosphere-keys").await,
                _ => return Err(anyhow!("Unsupported configuration!")),
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        match &self.configuration.security {
            NoosphereSecurity::Insecure { path, .. } => PlatformKeyStorage::new(path),
            _ => Err(anyhow!("Unsupported configuration!")),
        }
    }

    fn sphere_storage_path(&self) -> &PathBuf {
        match &self.configuration.storage {
            NoosphereStorage::Scoped { path } => path,
            NoosphereStorage::Unscoped { path } => path,
        }
    }

    fn gateway_api(&self) -> Option<&Url> {
        match &self.configuration.network {
            NoosphereNetwork::Http { gateway_api, .. } => gateway_api.as_ref(),
        }
    }

    fn ipfs_gateway_url(&self) -> Option<&Url> {
        match &self.configuration.network {
            NoosphereNetwork::Http {
                ipfs_gateway_url, ..
            } => ipfs_gateway_url.as_ref(),
        }
    }

    /// Create a key in the locally available platform key storage, associating
    /// it with the given human-readable key name
    pub async fn create_key(&self, key_name: &str) -> Result<()> {
        if key_name.is_empty() {
            return Err(anyhow!("Key name must not be empty."));
        }
        let key_storage = self.key_storage().await?;
        key_storage.create_key(key_name).await?;
        Ok(())
    }

    /// Check if a key has been created, given its human-readable key name
    pub async fn has_key(&self, key_name: &str) -> Result<bool> {
        let key_storage = self.key_storage().await?;
        Ok(key_storage.read_key(key_name).await?.is_some())
    }

    /// Create a sphere, generating an authorization for the specified owner key
    /// to administer the sphere over time
    pub async fn create_sphere(&self, owner_key_name: &str) -> Result<SphereReceipt> {
        let artifacts = SphereContextBuilder::default()
            .create_sphere()
            .at_storage_path(self.sphere_storage_path())
            .using_scoped_storage_layout()
            .reading_keys_from(self.key_storage().await?)
            .using_key(owner_key_name)
            .syncing_to(self.gateway_api())
            .reading_ipfs_from(self.ipfs_gateway_url())
            .build()
            .await?;

        let mnemonic = artifacts.require_mnemonic()?.to_owned();
        let context = SphereContext::from(artifacts);

        let sphere_identity = context.identity().to_owned();
        let mut sphere_contexts = self.sphere_channels.lock().await;
        sphere_contexts.insert(
            sphere_identity.clone(),
            SphereChannel::new(
                SphereCursor::latest(Arc::new(context.clone())),
                Arc::new(Mutex::new(context)),
            ),
        );

        Ok(SphereReceipt {
            identity: sphere_identity,
            mnemonic,
        })
    }

    /// Join a sphere by DID identity, given a local key and an [Authorization]
    /// proving that the key may operate on the sphere. This action will
    /// initalize the local sphere workspace, but none of the sphere data will
    /// be available until the local application syncs with a gateway that has
    /// the sphere data.
    pub async fn join_sphere(
        &self,
        sphere_identity: &Did,
        local_key_name: &str,
        authorization: Option<&Authorization>,
    ) -> Result<()> {
        let artifacts = SphereContextBuilder::default()
            .join_sphere(sphere_identity)
            .at_storage_path(self.sphere_storage_path())
            .using_scoped_storage_layout()
            .reading_keys_from(self.key_storage().await?)
            .using_key(local_key_name)
            .authorized_by(authorization)
            .syncing_to(self.gateway_api())
            .reading_ipfs_from(self.ipfs_gateway_url())
            .build()
            .await?;

        let context = SphereContext::from(artifacts);

        let sphere_identity = context.identity().to_owned();
        let mut sphere_contexts = self.sphere_channels.lock().await;
        sphere_contexts.insert(
            sphere_identity,
            SphereChannel::new(
                SphereCursor::latest(Arc::new(context.clone())),
                Arc::new(Mutex::new(context)),
            ),
        );

        Ok(())
    }

    /// Access a [SphereChannel] associated with the given sphere DID identity.
    /// The related sphere must already have been initialized locally (either by
    /// creating it or joining one that was created elsewhere). The act of
    /// creating or joining will initialize a [SphereChannel], but if such a
    /// channel has not already been initialized, accessing it with this method
    /// will cause it to be initialized and a reference kept by this
    /// [NoosphereContext].
    pub async fn get_sphere_channel(&self, sphere_identity: &Did) -> Result<PlatformSphereChannel> {
        let mut contexts = self.sphere_channels.lock().await;

        if !contexts.contains_key(sphere_identity) {
            let artifacts = SphereContextBuilder::default()
                .open_sphere(Some(sphere_identity))
                .at_storage_path(self.sphere_storage_path())
                .using_scoped_storage_layout()
                .reading_keys_from(self.key_storage().await?)
                .syncing_to(self.gateway_api())
                .reading_ipfs_from(self.ipfs_gateway_url())
                .build()
                .await?;

            let context = SphereContext::from(artifacts);
            contexts.insert(
                sphere_identity.to_owned(),
                SphereChannel::new(
                    SphereCursor::latest(Arc::new(context.clone())),
                    Arc::new(Mutex::new(context)),
                ),
            );
        }

        Ok(contexts
            .get(sphere_identity)
            .ok_or_else(|| anyhow!("Context was not initialized!"))?
            .clone())
    }
}
