mod create;
mod join;
mod open;
mod recover;

use create::*;
use join::*;
use open::*;
use recover::*;

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

use noosphere_core::{
    authority::Authorization,
    data::{Did, Mnemonic},
};

#[cfg(all(target_arch = "wasm32", feature = "ipfs-storage"))]
use noosphere_ipfs::{GatewayClient, IpfsStorage};

use noosphere_storage::SphereDb;

use url::Url;

use noosphere_core::context::SphereContext;

use crate::{
    platform::{PlatformKeyStorage, PlatformStorage},
    storage::StorageLayout,
};

#[derive(Default, Clone)]
enum SphereInitialization {
    #[default]
    Create,
    Join(Did),
    Recover(Did),
    Open(Option<Did>),
}

/// The effect of building a [SphereContext] with a [SphereContextBuilder] may
/// include artifacts besides the [SphereContext] that are relevant to the
/// workflow of the API user. This enum encapsulates the various results that
/// are possible.
pub enum SphereContextBuilderArtifacts {
    /// A sphere was newly created; the artifacts contain a [Mnemonic] which
    /// must be saved in some secure storage medium on the side (it will not
    /// be recorded in the user's sphere data).
    SphereCreated {
        /// A [SphereContext] for the newly created sphere
        context: SphereContext<PlatformStorage>,
        /// The recovery [Mnemonic] for the newly created sphere
        mnemonic: Mnemonic,
    },
    /// A sphere that existed prior to using the [SphereContextBuilder] has
    /// been opened for reading and writing.
    SphereOpened(SphereContext<PlatformStorage>),
}

impl SphereContextBuilderArtifacts {
    /// Attempt to read a [Mnemonic] from the artifact, returning an error
    /// result if no [Mnemonic] is present. Note that a [Mnemonic] is only
    /// expected to be present at the time that a sphere is newly created.
    pub fn require_mnemonic(&self) -> Result<&str> {
        match self {
            SphereContextBuilderArtifacts::SphereCreated { mnemonic, .. } => Ok(mnemonic.as_str()),
            _ => Err(anyhow!(
                "The sphere builder artifacts do not include a mnemonic!"
            )),
        }
    }
}

impl From<SphereContextBuilderArtifacts> for SphereContext<PlatformStorage> {
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
    pub(crate) scoped_storage_layout: bool,
    pub(crate) gateway_api: Option<Url>,
    pub(crate) ipfs_gateway_url: Option<Url>,
    pub(crate) storage_path: Option<PathBuf>,
    pub(crate) authorization: Option<Authorization>,
    pub(crate) key_storage: Option<PlatformKeyStorage>,
    pub(crate) key_name: Option<String>,
    pub(crate) mnemonic: Option<Mnemonic>,
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

    /// Configure this builder to recover an existing sphere's data from the gateway
    pub fn recover_sphere(mut self, sphere_id: &Did) -> Self {
        self.initialization = SphereInitialization::Recover(sphere_id.to_owned());
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

    /// Specify a [Mnemonic] to use; currently only used when recovering a
    /// sphere
    pub fn using_mnemonic(mut self, mnemonic: Option<&Mnemonic>) -> Self {
        self.mnemonic = mnemonic.cloned();
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
        let initialization = self.initialization.clone();
        match initialization {
            SphereInitialization::Create => create_a_sphere(self).await,
            SphereInitialization::Join(sphere_identity) => {
                join_a_sphere(self, sphere_identity).await
            }
            SphereInitialization::Recover(sphere_identity) => {
                recover_a_sphere(self, sphere_identity).await
            }
            SphereInitialization::Open(sphere_identity) => {
                open_a_sphere(self, sphere_identity).await
            }
        }
    }

    pub(crate) fn require_storage_path(&self) -> Result<&Path> {
        self.storage_path
            .as_deref()
            .ok_or_else(|| anyhow!("Storage path required but not configured"))
    }

    pub(crate) fn require_gateway_api(&self) -> Result<&Url> {
        self.gateway_api
            .as_ref()
            .ok_or_else(|| anyhow!("Gateway API URL required but not configured"))
    }

    pub(crate) fn require_mnemonic(&self) -> Result<&Mnemonic> {
        self.mnemonic
            .as_ref()
            .ok_or_else(|| anyhow!("Mnemonic required but not configured"))
    }

    pub(crate) fn require_key_storage(&self) -> Result<&PlatformKeyStorage> {
        self.key_storage
            .as_ref()
            .ok_or_else(|| anyhow!("Key storage required but not configured"))
    }

    pub(crate) fn require_key_name(&self) -> Result<&str> {
        self.key_name
            .as_deref()
            .ok_or_else(|| anyhow!("Key name required but not configured"))
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
            mnemonic: None,
        }
    }
}

impl TryFrom<(PathBuf, bool, Option<Did>)> for StorageLayout {
    type Error = anyhow::Error;

    fn try_from(
        (storage_path, scoped_storage_layout, sphere_identity): (PathBuf, bool, Option<Did>),
    ) -> std::result::Result<Self, Self::Error> {
        Ok(match scoped_storage_layout {
            true => StorageLayout::Scoped(
                storage_path,
                sphere_identity.ok_or_else(|| anyhow!("A sphere identity must be provided!"))?,
            ),
            false => StorageLayout::Unscoped(storage_path),
        })
    }
}

#[allow(unused_variables)]
pub(crate) async fn generate_db(
    storage_path: PathBuf,
    scoped_storage_layout: bool,
    sphere_identity: Option<Did>,
    ipfs_gateway_url: Option<Url>,
) -> Result<SphereDb<PlatformStorage>> {
    let storage_layout: StorageLayout =
        (storage_path, scoped_storage_layout, sphere_identity).try_into()?;

    #[cfg(not(all(wasm, ipfs_storage)))]
    let storage = storage_layout.to_storage().await?;
    #[cfg(all(wasm, ipfs_storage))]
    let storage = IpfsStorage::new(
        storage_layout.to_storage().await?,
        ipfs_gateway_url.map(|url| GatewayClient::new(url)),
    );

    SphereDb::new(storage).await
}

#[cfg(test)]
mod tests {
    use super::SphereContextBuilder;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    use crate::{key::KeyStorage, platform::make_temporary_platform_primitives};
    use noosphere_core::context::SphereContext;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_create_a_sphere_and_later_open_it() {
        let (storage_path, key_storage, _temporary_directories) =
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

            let sphere_context: SphereContext<_> = artifacts.into();
            sphere_context.identity().clone()
        };

        let context: SphereContext<_> = SphereContextBuilder::default()
            .open_sphere(None)
            .at_storage_path(&storage_path)
            .reading_keys_from(key_storage)
            .build()
            .await
            .unwrap()
            .into();

        assert_eq!(&sphere_identity, context.identity());
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_create_a_scoped_sphere_and_later_open_it() {
        let (storage_path, key_storage, _temporary_directories) =
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

            let sphere_context: SphereContext<_> = artifacts.into();
            sphere_context.identity().clone()
        };

        let context: SphereContext<_> = SphereContextBuilder::default()
            .open_sphere(Some(&sphere_identity))
            .using_scoped_storage_layout()
            .at_storage_path(&storage_path)
            .reading_keys_from(key_storage)
            .build()
            .await
            .unwrap()
            .into();

        assert_eq!(&sphere_identity, context.identity());
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_initialize_a_sphere_to_sync_from_elsewhere() {
        let (storage_path, key_storage, _temporary_directories) =
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

        let context: SphereContext<_> = artifacts.into();

        assert_eq!(context.identity().as_str(), "did:key:foo");
    }
}
