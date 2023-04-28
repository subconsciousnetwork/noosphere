use cid::Cid;
use noosphere_core::{authority::Authorization, tracing::initialize_tracing};
use url::Url;
use wasm_bindgen::prelude::*;

use crate::{
    wasm::SphereContext, NoosphereContext as NoosphereContextImpl, NoosphereContextConfiguration,
    NoosphereNetwork, NoosphereSecurity, NoosphereStorage,
};

#[wasm_bindgen]
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
    // NOTE: Cannot directly expose non-copy types to JS; instead must create an
    // explicit accessor to describe how to make a copy
    // SEE: https://github.com/rustwasm/wasm-bindgen/issues/1985#issuecomment-582092195
    #[wasm_bindgen(skip)]
    pub identity: String,
    #[wasm_bindgen(skip)]
    pub mnemonic: String,
}

#[wasm_bindgen]
impl SphereReceipt {
    #[wasm_bindgen(getter)]
    pub fn identity(&self) -> String {
        self.identity.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn mnemonic(&self) -> String {
        self.mnemonic.clone()
    }
}

#[wasm_bindgen]
/// A `NoosphereContext` is an application's gateway to interacting with the
/// Noosphere. It exposes an API for managing keys in a secure way, and also an
/// API for creating, opening and joining (that is, pairing new clients to)
/// spheres.
pub struct NoosphereContext {
    inner: NoosphereContextImpl,
}

#[wasm_bindgen]
impl NoosphereContext {
    #[wasm_bindgen(constructor)]
    pub fn new(
        storage_namespace: String,
        gateway_api: Option<String>,
        ipfs_gateway_url: Option<String>,
    ) -> Self {
        initialize_tracing(None);
        info!("Hello, Noosphere!");

        let gateway_api = if let Some(gateway_api) = gateway_api {
            Url::parse(&gateway_api).ok()
        } else {
            None
        };

        let ipfs_gateway_url = if let Some(ipfs_gateway_url) = ipfs_gateway_url {
            Url::parse(&ipfs_gateway_url).ok()
        } else {
            None
        };

        let noosphere_context = NoosphereContextImpl::new(NoosphereContextConfiguration {
            storage: NoosphereStorage::Scoped {
                path: storage_namespace.into(),
            },
            security: NoosphereSecurity::Opaque,
            network: NoosphereNetwork::Http {
                gateway_api,
                ipfs_gateway_url,
            },
        })
        .unwrap();

        NoosphereContext {
            inner: noosphere_context,
        }
    }

    #[wasm_bindgen(js_name = "createKey")]
    /// Create a new key, and assign it the given human-readable name
    pub async fn create_key(&self, key_name: String) {
        self.inner
            .create_key(&key_name)
            .await
            .unwrap_or_else(|error| error!("{:?}", error));
    }

    #[wasm_bindgen(js_name = "hasKey")]
    /// Check to see if a key with the given name exists in key storage
    pub async fn has_key(&self, key_name: String) -> Result<bool, String> {
        self.inner
            .has_key(&key_name)
            .await
            .map_err(|error| format!("{:?}", error))
    }

    #[wasm_bindgen(js_name = "createSphere")]
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

    #[wasm_bindgen(js_name = "joinSphere")]
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

    #[wasm_bindgen(js_name = "getSphereContext")]
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
