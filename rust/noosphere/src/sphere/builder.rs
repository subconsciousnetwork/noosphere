use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use cid::Cid;

use noosphere_core::{
    authority::{Author, Authorization},
    data::Did,
    view::Sphere,
};

#[cfg(all(target_arch = "wasm32", feature = "ipfs-storage"))]
mod ipfs_storage {
    pub use noosphere_ipfs::GatewayClient;
    pub use noosphere_storage::IpfsStorage;
}
#[cfg(all(target_arch = "wasm32", feature = "ipfs-storage"))]
use ipfs_storage::*;

use noosphere_storage::{KeyValueStore, MemoryStore, SphereDb};
use ucan::crypto::KeyMaterial;
use url::Url;

use crate::{
    key::KeyStorage,
    platform::{PlatformKeyMaterial, PlatformKeyStorage, PlatformStorage},
    sphere::{
        context::SphereContext,
        metadata::{AUTHORIZATION, IDENTITY, USER_KEY_NAME},
        storage::StorageLayout,
    },
};

enum SphereInitialization {
    Create,
    Join(Did),
    Open(Option<Did>),
}

impl Default for SphereInitialization {
    fn default() -> Self {
        SphereInitialization::Create
    }
}

/// The effect of building a [SphereContext] with a [SphereContextBuilder] may
/// include artifacts besides the [SphereContext] that are relevant to the
/// workflow of the API user. This enum encapsulates the various results that
/// are possible.
pub enum SphereContextBuilderArtifacts {
    SphereCreated {
        context: SphereContext<PlatformKeyMaterial, PlatformStorage>,
        mnemonic: String,
    },
    SphereOpened(SphereContext<PlatformKeyMaterial, PlatformStorage>),
}

impl SphereContextBuilderArtifacts {
    pub fn require_mnemonic(&self) -> Result<&str> {
        match self {
            SphereContextBuilderArtifacts::SphereCreated { mnemonic, .. } => Ok(mnemonic.as_str()),
            _ => Err(anyhow!(
                "The sphere builder artifacts do not include a mnemonic!"
            )),
        }
    }
}

impl From<SphereContextBuilderArtifacts> for SphereContext<PlatformKeyMaterial, PlatformStorage> {
    fn from(artifacts: SphereContextBuilderArtifacts) -> Self {
        match artifacts {
            SphereContextBuilderArtifacts::SphereCreated { context, .. } => context,
            SphereContextBuilderArtifacts::SphereOpened(context) => context,
        }
    }
}

/// A [SphereContextBuilder] is a common entrypoint for initializing a
/// [SphereContext]. It embodies various workflows that may result in a sphere
/// being activated for use by the embedding application, including: creating a
/// new sphere, joining an existing sphere or accessing a sphere that has
/// already been created or joined.
pub struct SphereContextBuilder {
    initialization: SphereInitialization,
    scoped_storage_layout: bool,
    gateway_api: Option<Url>,
    ipfs_gateway_url: Option<Url>,
    storage_path: Option<PathBuf>,
    authorization: Option<Authorization>,
    key_storage: Option<PlatformKeyStorage>,
    key_name: Option<String>,
}

impl SphereContextBuilder {
    /// Configure this builder to join a sphere by some DID identity
    pub fn join_sphere(mut self, sphere_identity: &Did) -> Self {
        self.initialization = SphereInitialization::Join(sphere_identity.to_owned());
        self
    }

    /// Configure this builder to create a new sphere
    pub fn create_sphere(mut self) -> Self {
        self.initialization = SphereInitialization::Create;
        self
    }

    /// Configure this builder to open an existing sphere that was previously
    /// created or joined; if the sphere uses a scoped layout, you must provide
    /// the identity of the sphere as well.
    pub fn open_sphere(mut self, sphere_identity: Option<&Did>) -> Self {
        self.initialization = SphereInitialization::Open(sphere_identity.cloned());
        self
    }

    /// Specify the URL of a gateway API for this application to sync sphere
    /// data with
    pub fn syncing_to(mut self, gateway_url: Option<&Url>) -> Self {
        self.gateway_api = gateway_url.cloned();
        self
    }

    /// Specify the URL of an IPFS HTTP Gateway for this application to access
    /// as a contingency when blocks being read from storage are missing
    pub fn reading_ipfs_from(mut self, ipfs_gateway_url: Option<&Url>) -> Self {
        self.ipfs_gateway_url = ipfs_gateway_url.cloned();
        self
    }

    /// When initializing sphere data, scope the namespace by the sphere's DID
    pub fn using_scoped_storage_layout(mut self) -> Self {
        self.scoped_storage_layout = true;
        self
    }

    /// Specify the local namespace in storage where sphere data should be
    /// initialized
    pub fn at_storage_path(mut self, path: &Path) -> Self {
        self.storage_path = Some(path.into());
        self
    }

    /// Specify the authorization that enables a local key to manipulate a
    /// sphere
    pub fn authorized_by(mut self, authorization: Option<&Authorization>) -> Self {
        self.authorization = authorization.cloned();
        self
    }

    /// Specify the key storage backend (a [KeyStorage] implementation) that
    /// manages keys on behalf of the local user
    pub fn reading_keys_from(mut self, key_storage: PlatformKeyStorage) -> Self {
        self.key_storage = Some(key_storage);
        self
    }

    /// Specify the name that is associated with a user key in a configured
    /// [KeyStorage] backend
    pub fn using_key(mut self, key_name: &str) -> Self {
        self.key_name = Some(key_name.to_owned());
        self
    }

    /// Generate [SphereContextBuilderArtifacts] based on the given
    /// configuration of the [SphereContextBuilder]. The successful result of
    /// invoking this method will always include an activated [SphereContext].
    /// It will also cause a namespace hierarchy and local data that is
    /// is associated with a sphere to exist if it doesn't already. So, consider
    /// invocations of this API to have side-effects that may need undoing if
    /// idempotence is required (e.g., in tests).
    pub async fn build(self) -> Result<SphereContextBuilderArtifacts> {
        let storage_path = self
            .storage_path
            .ok_or_else(|| anyhow!("No storage path configured!"))?;
        match self.initialization {
            SphereInitialization::Create => {
                let key_storage = self
                    .key_storage
                    .as_ref()
                    .ok_or_else(|| anyhow!("No key storage configured!"))?;
                let key_name = self
                    .key_name
                    .as_ref()
                    .ok_or_else(|| anyhow!("No key name configured!"))?;
                if self.authorization.is_some() {
                    warn!("Creating a new sphere; the configured authorization will be ignored!");
                }

                let owner_key = key_storage.require_key(key_name).await?;
                let owner_did = owner_key.get_did().await?;

                let mut memory_store = MemoryStore::default();
                let (sphere, authorization, mnemonic) =
                    Sphere::try_generate(&owner_did, &mut memory_store)
                        .await
                        .unwrap();

                let sphere_did = sphere.try_get_identity().await.unwrap();
                let mut db = generate_db(
                    storage_path,
                    self.scoped_storage_layout,
                    Some(sphere_did.clone()),
                    self.ipfs_gateway_url,
                )
                .await?;

                db.persist(&memory_store).await?;

                db.set_version(&sphere_did, sphere.cid()).await?;

                db.set_key(IDENTITY, &sphere_did).await?;
                db.set_key(USER_KEY_NAME, key_name.to_owned()).await?;
                db.set_key(AUTHORIZATION, Cid::try_from(&authorization)?)
                    .await?;

                let mut context = SphereContext::new(
                    sphere_did,
                    Author {
                        key: owner_key,
                        authorization: Some(authorization),
                    },
                    db,
                );

                if self.gateway_api.is_some() {
                    context
                        .configure_gateway_url(self.gateway_api.as_ref())
                        .await?;
                }

                Ok(SphereContextBuilderArtifacts::SphereCreated { context, mnemonic })
            }
            SphereInitialization::Join(sphere_identity) => {
                let key_storage = self
                    .key_storage
                    .as_ref()
                    .ok_or_else(|| anyhow!("No key storage configured!"))?;
                let key_name = self
                    .key_name
                    .as_ref()
                    .ok_or_else(|| anyhow!("No key name configured!"))?;

                let user_key = key_storage.require_key(key_name).await?;
                let mut db = generate_db(
                    storage_path,
                    self.scoped_storage_layout,
                    Some(sphere_identity.clone()),
                    self.ipfs_gateway_url,
                )
                .await?;

                db.set_key(IDENTITY, &sphere_identity).await?;
                db.set_key(USER_KEY_NAME, key_name.to_owned()).await?;

                if let Some(authorization) = &self.authorization {
                    db.set_key(AUTHORIZATION, Cid::try_from(authorization)?)
                        .await?;
                }

                let mut context = SphereContext::new(
                    sphere_identity,
                    Author {
                        key: user_key,
                        authorization: self.authorization,
                    },
                    db,
                );

                if self.gateway_api.is_some() {
                    context
                        .configure_gateway_url(self.gateway_api.as_ref())
                        .await?;
                }

                Ok(SphereContextBuilderArtifacts::SphereOpened(context))
            }
            SphereInitialization::Open(sphere_identity) => {
                let db = generate_db(
                    storage_path,
                    self.scoped_storage_layout,
                    sphere_identity,
                    self.ipfs_gateway_url,
                )
                .await?;

                let user_key_name: String = db.require_key(USER_KEY_NAME).await?;
                let authorization = db
                    .get_key(AUTHORIZATION)
                    .await?
                    .map(|cid| Authorization::Cid(cid));

                let author = match self.key_storage {
                    Some(key_storage) => Author {
                        key: key_storage.require_key(&user_key_name).await?,
                        authorization,
                    },
                    _ => return Err(anyhow!("Unable to resolve sphere author")),
                };

                let sphere_identity = db.require_key(IDENTITY).await?;
                let mut context = SphereContext::new(sphere_identity, author, db);

                if self.gateway_api.is_some() {
                    context
                        .configure_gateway_url(self.gateway_api.as_ref())
                        .await?;
                }

                Ok(SphereContextBuilderArtifacts::SphereOpened(context))
            }
        }
    }
}

impl Default for SphereContextBuilder {
    fn default() -> Self {
        Self {
            initialization: SphereInitialization::Create,
            scoped_storage_layout: false,
            gateway_api: None,
            ipfs_gateway_url: None,
            storage_path: None,
            authorization: None,
            key_storage: None as Option<PlatformKeyStorage>,
            key_name: None,
        }
    }
}

#[allow(unused_variables)]
async fn generate_db(
    storage_path: PathBuf,
    scoped_storage_layout: bool,
    sphere_identity: Option<Did>,
    ipfs_gateway_url: Option<Url>,
) -> Result<SphereDb<PlatformStorage>> {
    let storage_layout = match scoped_storage_layout {
        true => StorageLayout::Scoped(
            storage_path,
            sphere_identity.ok_or_else(|| anyhow!("A sphere identity must be provided!"))?,
        ),
        false => StorageLayout::Unscoped(storage_path),
    };

    #[cfg(not(all(target_arch = "wasm32", feature = "ipfs-storage")))]
    let storage = storage_layout.to_storage().await?;
    #[cfg(all(target_arch = "wasm32", feature = "ipfs-storage"))]
    let storage = IpfsStorage::new(
        storage_layout.to_storage().await?,
        ipfs_gateway_url.map(|url| GatewayClient::new(url)),
    );

    SphereDb::new(&storage).await
}

#[cfg(test)]
mod tests {
    use super::SphereContextBuilder;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    use crate::{
        key::KeyStorage, platform::make_temporary_platform_primitives, sphere::SphereContext,
    };

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_create_a_sphere_and_later_open_it() {
        let (storage_path, key_storage, temporary_directories) =
            make_temporary_platform_primitives().await.unwrap();

        key_storage.create_key("foo").await.unwrap();

        let sphere_identity = {
            let artifacts = SphereContextBuilder::default()
                .create_sphere()
                .at_storage_path(&storage_path)
                .reading_keys_from(key_storage.clone())
                .using_key("foo")
                .build()
                .await
                .unwrap();

            artifacts.require_mnemonic().unwrap();

            let sphere_context: SphereContext<_, _> = artifacts.into();
            sphere_context.identity().clone()
        };

        let context: SphereContext<_, _> = SphereContextBuilder::default()
            .open_sphere(None)
            .at_storage_path(&storage_path)
            .reading_keys_from(key_storage)
            .build()
            .await
            .unwrap()
            .into();

        assert_eq!(&sphere_identity, context.identity());

        drop(temporary_directories);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_create_a_scoped_sphere_and_later_open_it() {
        let (storage_path, key_storage, temporary_directories) =
            make_temporary_platform_primitives().await.unwrap();

        key_storage.create_key("foo").await.unwrap();

        let sphere_identity = {
            let artifacts = SphereContextBuilder::default()
                .create_sphere()
                .using_scoped_storage_layout()
                .at_storage_path(&storage_path)
                .reading_keys_from(key_storage.clone())
                .using_key("foo")
                .build()
                .await
                .unwrap();

            artifacts.require_mnemonic().unwrap();

            let sphere_context: SphereContext<_, _> = artifacts.into();
            sphere_context.identity().clone()
        };

        let context: SphereContext<_, _> = SphereContextBuilder::default()
            .open_sphere(Some(&sphere_identity))
            .using_scoped_storage_layout()
            .at_storage_path(&storage_path)
            .reading_keys_from(key_storage)
            .build()
            .await
            .unwrap()
            .into();

        assert_eq!(&sphere_identity, context.identity());

        drop(temporary_directories);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_initialize_a_sphere_to_sync_from_elsewhere() {
        let (storage_path, key_storage, temporary_directories) =
            make_temporary_platform_primitives().await.unwrap();

        key_storage.create_key("foo").await.unwrap();

        let artifacts = SphereContextBuilder::default()
            .join_sphere(&"did:key:foo".into())
            .at_storage_path(&storage_path)
            .reading_keys_from(key_storage)
            .authorized_by(None)
            .using_key("foo")
            .build()
            .await
            .unwrap();

        let context: SphereContext<_, _> = artifacts.into();

        assert_eq!(context.identity().as_str(), "did:key:foo");

        drop(temporary_directories);
    }
}
