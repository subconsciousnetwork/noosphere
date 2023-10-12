//! Intermediate constructs to normalize how storage is initialized

use crate::platform::PlatformStorage;
use anyhow::Result;
use noosphere_core::data::Did;
use noosphere_storage::StorageConfig;
use std::{
    fmt::Display,
    path::{Path, PathBuf},
};
use url::Url;

#[cfg(doc)]
use noosphere_storage::Storage;

#[cfg(feature = "ipfs-storage")]
use noosphere_ipfs::{GatewayClient, IpfsStorage};

/// [StorageLayout] represents the namespace that should be used depending on
/// whether or not a sphere's DID should be included in the namespace. The enum
/// is a convenience that can be directly transformed into a [Storage]
/// implementation that is suitable for the current platform
pub enum StorageLayout {
    /// Storage will be automatically scoped by the [Did] of the sphere
    Scoped(PathBuf, Did),
    /// Storage will be initialized in a .sphere folder within the configured
    /// path
    Unscoped(PathBuf),
}

impl Display for StorageLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let path = PathBuf::from(self);

        write!(f, "{}", path.to_string_lossy())
    }
}

impl From<&StorageLayout> for PathBuf {
    fn from(layout: &StorageLayout) -> Self {
        match layout {
            StorageLayout::Scoped(path, scope) => get_scoped_path(path, scope),
            StorageLayout::Unscoped(path) => path.join(".sphere/storage"),
        }
    }
}

impl From<StorageLayout> for PathBuf {
    fn from(layout: StorageLayout) -> Self {
        PathBuf::from(&layout)
    }
}

fn get_scoped_path(path: &Path, scope: &Did) -> PathBuf {
    #[cfg(not(windows))]
    let path_buf = path.join(scope.as_str());

    #[cfg(windows)]
    // Windows does not allow `:` in file paths.
    // Replace `:` in scope/key with `_`.
    let path_buf = path.join(scope.as_str().replace(":", "_"));

    path_buf
}

/// Construct [PlatformStorage] from a [StorageLayout] and [StorageConfig].
///
/// Takes a [Url] to an IPFS Gateway that is used when compiling with `ipfs-storage`.
pub async fn create_platform_storage(
    layout: StorageLayout,
    #[allow(unused)] ipfs_gateway_url: Option<Url>,
    #[allow(unused)] storage_config: Option<StorageConfig>,
) -> Result<PlatformStorage> {
    #[cfg(any(sled, rocksdb))]
    let storage = {
        use noosphere_storage::ConfigurableStorage;
        let path: PathBuf = layout.into();
        crate::platform::PrimitiveStorage::open_with_config(
            &path,
            storage_config.unwrap_or_default(),
        )
        .await?
    };

    #[cfg(wasm)]
    let storage = noosphere_storage::IndexedDbStorage::new(&layout.to_string()).await?;

    #[cfg(ipfs_storage)]
    let storage = {
        let maybe_client = ipfs_gateway_url.map(|url| GatewayClient::new(url));
        IpfsStorage::new(storage, maybe_client)
    };

    debug!("Created platform storage: {:#?}", storage);

    Ok(storage)
}
