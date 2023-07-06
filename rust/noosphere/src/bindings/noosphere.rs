use cid::Cid;
use noosphere_core::{authority::Authorization, tracing::initialize_tracing};
use url::Url;

use crate::bindings::SphereContext;
use crate::implementation::{
    NoosphereContext as NoosphereContextImpl, NoosphereContextConfiguration, NoosphereError,
    NoosphereNetwork, NoosphereSecurity, NoosphereStorage,
};
/// A `SphereReceipt` is provided when a sphere has been successfully created.
/// It contains the unique identity of the sphere (a [DID
/// Key](https://w3c-ccg.github.io/did-method-key/) string), as well as a
/// mnemonic recovery string.
///
/// The identity is needed in order to retrieve the related `SphereContext`
/// whenever the user wants to read from or write to the sphere. It is not
/// secret information, and can be stored in plain text.
///
/// The returned mnemonic is highly sensitive; it can be used to rotate the
/// authorizations that enable a user to write to the sphere. It is intended
/// that the user who the sphere has been created for will keep the mnemonic in
/// a secure location (such as a password manager) to be used in the future when
/// account recovery or migration is called for.
pub struct SphereReceipt {
    pub identity: String,
    pub mnemonic: String,
}

impl SphereReceipt {
    pub fn identity(&self) -> String {
        self.identity.clone()
    }

    pub fn mnemonic(&self) -> String {
        self.mnemonic.clone()
    }
}

/// A `NoosphereContext` is an application's gateway to interacting with the
/// Noosphere. It exposes an API for managing keys in a secure way, and also an
/// API for creating, opening and joining (that is, pairing new clients to)
/// spheres.
pub struct NoosphereContext {
    inner: NoosphereContextImpl,
}

impl NoosphereContext {
    pub fn new<S: Into<String>, U: Into<Url>>(
        global_storage_path: S,
        sphere_storage_path: S,
        gateway_api: Option<U>,
    ) -> Result<Self, NoosphereError> {
        initialize_tracing(None);
        info!("Hello, Noosphere!");
        Ok(NoosphereContext {
            inner: NoosphereContextImpl::new(NoosphereContextConfiguration {
                security: NoosphereSecurity::Insecure {
                    path: global_storage_path.into().into(),
                },
                storage: NoosphereStorage::Scoped {
                    path: sphere_storage_path.into().into(),
                },
                network: NoosphereNetwork::Http {
                    gateway_api: gateway_api.map(|u| u.into()),
                    ipfs_gateway_url: None,
                },
            })?,
        })
    }

    /// Create a new key, and assign it the given human-readable name
    pub async fn create_key(&self, key_name: String) -> Result<(), NoosphereError> {
        self.inner
            .create_key(&key_name)
            .await
            .map_err(|e| <anyhow::Error as Into<NoosphereError>>::into(e))
    }

    /// Check to see if a key with the given name exists in key storage
    pub async fn has_key(&self, key_name: String) -> Result<bool, NoosphereError> {
        self.inner
            .has_key(&key_name)
            .await
            .map_err(|e| <anyhow::Error as Into<NoosphereError>>::into(e))
    }

    /// Create a new sphere, assigning the key (given by its human-readable name)
    /// as the authorized key for writing changes to the sphere moving forward.
    pub async fn create_sphere(&self, key: String) -> Result<SphereReceipt, String> {
        let receipt = self
            .inner
            .create_sphere(&key)
            .await
            .map_err(|error| format!("{:?}", error))?;

        Ok(SphereReceipt {
            identity: receipt.identity.into(),
            mnemonic: receipt.mnemonic,
        })
    }

    /// Join an existing sphere given its identity, using the local key (given
    /// by its human-readable name) and optional authorization (given by a
    /// base64-encoded CID) as the credentials that would give the local user
    /// access to the sphere. Note that once you have joined a sphere, you must
    /// sync it with a gateway before you can access its data.
    pub async fn join_sphere(
        &self,
        identity: String,
        key: String,
        authorization: Option<String>,
    ) -> Result<(), String> {
        let authorization = match authorization {
            Some(cid_string) => Some(Authorization::Cid(
                Cid::try_from(cid_string).map_err(|error| format!("{:?}", error))?,
            )),
            None => None,
        };

        self.inner
            .join_sphere(&identity.into(), &key, authorization.as_ref())
            .await
            .map_err(|error| format!("{:?}", error))?;

        Ok(())
    }

    /// Access a `SphereContext` that was either created on this device, or
    /// joined so that it can be replicated on this device.
    pub async fn get_sphere_context(&self, identity: String) -> Result<SphereContext, String> {
        Ok(SphereContext {
            inner: self
                .inner
                .get_sphere_channel(&identity.into())
                .await
                .map_err(|error| format!("{:?}", error))?,
        })
    }
}
