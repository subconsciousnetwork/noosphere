use std::{fmt::Display, path::PathBuf};

use crate::platform::PlatformStorage;
use anyhow::Result;
use noosphere_core::data::Did;

#[cfg(doc)]
use noosphere_storage::Storage;

/// [StorageLayout] represents the namespace that should be used depending on
/// whether or not a sphere's DID should be included in the namespace. The enum
/// is a convenience that can be directly transformed into a [Storage]
/// implementation that is suitable for the current platform
pub enum StorageLayout {
    Scoped(PathBuf, Did),
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
            StorageLayout::Scoped(path, scope) => path.join(scope.as_str()),
            StorageLayout::Unscoped(path) => path.join(".sphere/storage"),
        }
    }
}

impl From<StorageLayout> for PathBuf {
    fn from(layout: StorageLayout) -> Self {
        PathBuf::from(&layout)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl StorageLayout {
    pub async fn to_storage_provider(&self) -> Result<PlatformStorage> {
        PlatformStorage::new(noosphere_storage::NativeStorageInit::Path(PathBuf::from(
            self,
        )))
    }
}

#[cfg(target_arch = "wasm32")]
impl StorageLayout {
    pub async fn to_storage_provider(&self) -> Result<PlatformStorage> {
        PlatformStorage::new(&self.to_string()).await
    }
}
