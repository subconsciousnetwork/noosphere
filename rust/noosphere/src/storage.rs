//! Intermediate constructs to normalize how storage is initialized

use crate::platform::PrimitiveStorage;
use anyhow::Result;
use noosphere_core::data::Did;
use std::{
    fmt::Display,
    path::{Path, PathBuf},
};

#[cfg(doc)]
use noosphere_storage::Storage;

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

#[cfg(native)]
impl StorageLayout {
    pub(crate) async fn to_storage(&self) -> Result<PrimitiveStorage> {
        #[cfg(sled)]
        {
            noosphere_storage::SledStorage::new(PathBuf::from(self))
        }
        #[cfg(rocksdb)]
        {
            noosphere_storage::RocksDbStorage::new(PathBuf::from(self)).await
        }
    }
}

#[cfg(wasm)]
impl StorageLayout {
    /// Convert this [StorageLayout] to a [noosphere_storage::Storage] based on the
    /// defaults configured for the current platform.
    pub async fn to_storage(&self) -> Result<PrimitiveStorage> {
        noosphere_storage::IndexedDbStorage::new(&self.to_string()).await
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
