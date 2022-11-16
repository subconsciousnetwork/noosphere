use anyhow::{anyhow, Result};
use noosphere_core::{authority::Authorization, data::Did};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use tokio::sync::Mutex;
use url::Url;

use crate::{
    key::KeyStorage,
    platform::{PlatformKeyMaterial, PlatformKeyStorage, PlatformStore},
    sphere::{SphereContext, SphereContextBuilder, SphereReceipt},
};

/// This enum exists so that we can incrementally layer on support for secure
/// key storage over time. Each member represents a set of environmental
/// qualities, with the most basic represnting an environment with no trusted
/// hardware key storage.
#[derive(Clone)]
pub enum NoosphereContextConfiguration {
    /// Insecure configuration should be used on a platform where no TPM or
    /// similar secure key storage is available.
    Insecure {
        global_storage_path: PathBuf,
        sphere_storage_path: PathBuf,
        gateway_url: Option<Url>,
    },

    /// Opaque security configuration may be used in the case where there is
    /// some kind of protected keyring-like API layer where secret key material
    /// may be considered safely stored. For example: secret service on Linux
    /// or the keyring on MacOS.
    OpaqueSecurity {
        sphere_storage_path: PathBuf,
        gateway_url: Option<Url>,
    },
}

/// A [NoosphereContext] holds configuration necessary to initialize and store
/// Noosphere data. It also keeps a running list of active [SphereContext]
/// instances to avoid the expensive action of repeatedly opening and closing
/// a handle to backing storage for spheres that are being accessed regularly.
pub struct NoosphereContext {
    configuration: NoosphereContextConfiguration,
    sphere_contexts:
        Arc<Mutex<BTreeMap<Did, Arc<Mutex<SphereContext<PlatformKeyMaterial, PlatformStore>>>>>>,
}

impl NoosphereContext {
    /// Initialize a [NoosphereContext] with a [NoosphereContextConfiguration]
    pub fn new(configuration: NoosphereContextConfiguration) -> Result<Self> {
        Ok(NoosphereContext {
            configuration,
            sphere_contexts: Default::default(),
        })
    }

    async fn key_storage(&self) -> Result<PlatformKeyStorage> {
        #[cfg(target_arch = "wasm32")]
        {
            match &self.configuration {
                NoosphereContextConfiguration::OpaqueSecurity { .. } => {
                    PlatformKeyStorage::new("noosphere-keys").await
                }
                _ => return Err(anyhow!("Unsupported configuration!")),
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        match &self.configuration {
            NoosphereContextConfiguration::Insecure {
                global_storage_path,
                ..
            } => PlatformKeyStorage::new(global_storage_path),
            _ => Err(anyhow!("Unsupported configuration!")),
        }
    }

    fn sphere_storage_path(&self) -> &PathBuf {
        match &self.configuration {
            NoosphereContextConfiguration::Insecure {
                sphere_storage_path,
                ..
            } => sphere_storage_path,
            NoosphereContextConfiguration::OpaqueSecurity {
                sphere_storage_path,
                ..
            } => sphere_storage_path,
        }
    }

    fn gateway_url(&self) -> Option<&Url> {
        match &self.configuration {
            NoosphereContextConfiguration::Insecure { gateway_url, .. } => gateway_url.as_ref(),
            NoosphereContextConfiguration::OpaqueSecurity { gateway_url, .. } => {
                gateway_url.as_ref()
            }
        }
    }

    /// Create a key in the locally available platform key storage, associating
    /// it with the given human-readable key name
    pub async fn create_key(&self, key_name: &str) -> Result<()> {
        let key_storage = self.key_storage().await?;
        key_storage.create_key(key_name).await?;
        Ok(())
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
            .syncing_to(self.gateway_url())
            .build()
            .await?;

        let mnemonic = artifacts.require_mnemonic()?.to_owned();
        let context = SphereContext::from(artifacts);

        let sphere_identity = context.identity().to_owned();
        let mut sphere_contexts = self.sphere_contexts.lock().await;
        sphere_contexts.insert(sphere_identity.clone(), Arc::new(Mutex::new(context)));

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
        authorization: &Authorization,
    ) -> Result<()> {
        let artifacts = SphereContextBuilder::default()
            .join_sphere(sphere_identity)
            .at_storage_path(self.sphere_storage_path())
            .using_scoped_storage_layout()
            .reading_keys_from(self.key_storage().await?)
            .using_key(local_key_name)
            .authorized_by(authorization)
            .syncing_to(self.gateway_url())
            .build()
            .await?;

        let context = SphereContext::from(artifacts);

        let sphere_identity = context.identity().to_owned();
        let mut sphere_contexts = self.sphere_contexts.lock().await;
        sphere_contexts.insert(sphere_identity, Arc::new(Mutex::new(context)));

        Ok(())
    }

    /// Access a [SphereContext] associated with the given sphere DID identity.
    /// The sphere must already have been initialized locally (either by
    /// creating it or joining one that was created elsewhere). The act of
    /// creating or joining will initialize a [SphereContext], but if such a
    /// context has not already been initialized, accessing it with this method
    /// will cause it to be initialized and a reference kept by this
    /// [NoosphereContext].
    pub async fn get_sphere_context(
        &self,
        sphere_identity: &Did,
    ) -> Result<Arc<Mutex<SphereContext<PlatformKeyMaterial, PlatformStore>>>> {
        let mut contexts = self.sphere_contexts.lock().await;

        if !contexts.contains_key(sphere_identity) {
            let artifacts = SphereContextBuilder::default()
                .open_sphere(Some(sphere_identity))
                .at_storage_path(self.sphere_storage_path())
                .using_scoped_storage_layout()
                .reading_keys_from(self.key_storage().await?)
                .syncing_to(self.gateway_url())
                .build()
                .await?;

            let context = SphereContext::from(artifacts);

            contexts.insert(sphere_identity.to_owned(), Arc::new(Mutex::new(context)));
        }

        Ok(contexts
            .get(sphere_identity)
            .ok_or_else(|| anyhow!("Context was not initialized!"))?
            .clone())
    }
}
