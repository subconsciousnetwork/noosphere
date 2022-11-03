use std::{path::PathBuf, sync::Arc};

use anyhow::{anyhow, Result};
use cid::Cid;

use noosphere_core::{authority::Authorization, view::Sphere};

use noosphere_storage::{db::SphereDb, interface::KeyValueStore, memory::MemoryStore};
use ucan::crypto::KeyMaterial;
use url::Url;

use crate::{
    key::KeyStorage,
    platform::{PlatformKeyMaterial, PlatformKeyStorage, PlatformStore},
    sphere::{
        access::SphereAccess,
        context::SphereContext,
        storage::{StorageLayout, AUTHORIZATION, USER_KEY_NAME},
    },
};

enum SphereInitialization {
    Create,
    Join(String),
    Open(String),
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
        context: SphereContext<PlatformKeyMaterial, PlatformStore>,
        mnemonic: String,
    },
    SphereOpened(SphereContext<PlatformKeyMaterial, PlatformStore>),
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

impl From<SphereContextBuilderArtifacts> for SphereContext<PlatformKeyMaterial, PlatformStore> {
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
    gateway_url: Option<Url>,
    storage_path: Option<PathBuf>,
    authorization: Option<Authorization>,
    key_storage: Option<PlatformKeyStorage>,
    key_name: Option<String>,
}

impl SphereContextBuilder {
    /// Configure this builder to join a sphere by some DID identity
    pub fn join_sphere(mut self, sphere_identity: &str) -> Self {
        self.initialization = SphereInitialization::Join(sphere_identity.into());
        self
    }

    /// Configure this builder to create a new sphere
    pub fn create_sphere(mut self) -> Self {
        self.initialization = SphereInitialization::Create;
        self
    }

    /// Configure this builder to open an existing sphere that was previously
    /// created or joined
    pub fn open_sphere(mut self, sphere_identity: &str) -> Self {
        self.initialization = SphereInitialization::Open(sphere_identity.into());
        self
    }

    /// Specify the URL of a gateway API for this application to sync sphere
    /// data with
    pub fn syncing_to(mut self, gateway_url: Option<&Url>) -> Self {
        self.gateway_url = gateway_url.cloned();
        self
    }

    /// When initializing sphere data, scope the namespace by the sphere's DID
    pub fn using_scoped_storage_layout(mut self) -> Self {
        self.scoped_storage_layout = true;
        self
    }

    /// Specify the local namespace in storage where sphere data should be
    /// initialized
    pub fn at_storage_path(mut self, path: &PathBuf) -> Self {
        self.storage_path = Some(path.clone());
        self
    }

    /// Specify the authorization that enables a local key to manipulate a
    /// sphere
    pub fn authorized_by(mut self, authorization: &Authorization) -> Self {
        self.authorization = Some(authorization.clone());
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
        let storage_path = match self.storage_path {
            Some(storage_path) => storage_path,
            None => return Err(anyhow!("No storage path configured!")),
        };

        match self.initialization {
            SphereInitialization::Create => {
                let key_storage: PlatformKeyStorage = match self.key_storage {
                    Some(key_storage) => key_storage,
                    None => return Err(anyhow!("No key storage configured!")),
                };

                let key_name = match self.key_name {
                    Some(key_name) => key_name,
                    None => return Err(anyhow!("No key name configured!")),
                };

                if self.authorization.is_some() {
                    warn!("Creating a new sphere; the configured authorization will be ignored!");
                }

                let owner_key = key_storage.require_key(&key_name).await?;
                let owner_did = owner_key.get_did().await?;

                let mut memory_store = MemoryStore::default();
                let (sphere, authorization, mnemonic) =
                    Sphere::try_generate(&owner_did, &mut memory_store)
                        .await
                        .unwrap();

                let sphere_did = sphere.try_get_identity().await.unwrap();

                let storage_layout = match self.scoped_storage_layout {
                    true => StorageLayout::Scoped(storage_path, sphere_did.clone()),
                    false => StorageLayout::Unscoped(storage_path),
                };

                let storage_provider = storage_layout.to_storage_provider().await?;

                let mut db = SphereDb::new(&storage_provider).await?;

                db.persist(&memory_store).await?;

                db.set_version(&sphere_did, sphere.cid()).await?;

                db.set_key(USER_KEY_NAME, key_name).await?;
                db.set_key(AUTHORIZATION, Cid::try_from(&authorization)?)
                    .await?;

                Ok(SphereContextBuilderArtifacts::SphereCreated {
                    context: SphereContext::new(
                        sphere_did,
                        SphereAccess::ReadWrite {
                            user_key: Arc::new(owner_key),
                            user_identity: owner_did.clone(),
                            authorization,
                        },
                        db,
                        self.gateway_url,
                    ),
                    mnemonic,
                })
            }
            SphereInitialization::Join(sphere_identity) => {
                let key_storage = match self.key_storage {
                    Some(key_storage) => key_storage,
                    None => return Err(anyhow!("No key storage configured!")),
                };

                let key_name = match self.key_name {
                    Some(key_name) => key_name,
                    None => return Err(anyhow!("No key name configured!")),
                };

                let authorization = match self.authorization {
                    Some(authorization) => authorization,
                    None => return Err(anyhow!("No authorization configured!")),
                };

                let user_key = key_storage.require_key(&key_name).await?;
                let user_identity = user_key.get_did().await?;

                let storage_layout = match self.scoped_storage_layout {
                    true => StorageLayout::Scoped(storage_path, sphere_identity.clone()),
                    false => StorageLayout::Unscoped(storage_path),
                };

                let storage_provider = storage_layout.to_storage_provider().await?;

                let mut db = SphereDb::new(&storage_provider).await?;

                db.set_key(USER_KEY_NAME, key_name).await?;
                db.set_key(AUTHORIZATION, Cid::try_from(&authorization)?)
                    .await?;

                Ok(SphereContextBuilderArtifacts::SphereOpened(
                    SphereContext::new(
                        sphere_identity,
                        SphereAccess::ReadWrite {
                            user_key: Arc::new(user_key),
                            user_identity,
                            authorization,
                        },
                        db,
                        self.gateway_url,
                    ),
                ))
            }
            SphereInitialization::Open(sphere_identity) => {
                let storage_layout = match self.scoped_storage_layout {
                    true => StorageLayout::Scoped(storage_path, sphere_identity.clone()),
                    false => StorageLayout::Unscoped(storage_path),
                };

                let storage_provider = storage_layout.to_storage_provider().await?;
                let db = SphereDb::new(&storage_provider).await?;

                let access = match (
                    self.key_storage,
                    db.get_key(USER_KEY_NAME).await? as Option<String>,
                    db.get_key(AUTHORIZATION).await?,
                ) {
                    (Some(key_storage), Some(user_key_name), Some(cid)) => {
                        let user_key = key_storage.require_key(&user_key_name).await?;
                        let user_identity = user_key.get_did().await?;

                        SphereAccess::ReadWrite {
                            user_key: Arc::new(user_key),
                            user_identity,
                            authorization: Authorization::Cid(cid),
                        }
                    }
                    _ => SphereAccess::ReadOnly,
                };

                Ok(SphereContextBuilderArtifacts::SphereOpened(
                    SphereContext::new(sphere_identity, access, db, self.gateway_url),
                ))
            }
        }
    }
}

impl Default for SphereContextBuilder {
    fn default() -> Self {
        Self {
            initialization: SphereInitialization::Create,
            scoped_storage_layout: false,
            gateway_url: None,
            storage_path: None,
            authorization: None,
            key_storage: None as Option<PlatformKeyStorage>,
            key_name: None,
        }
    }
}
