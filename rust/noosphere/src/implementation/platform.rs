///! Platform-specific types and bindings
///! Platforms will vary in capabilities for things like block storage and
///! secure key management. This module lays out the concrete strategies we will
///! use on a per-platform basis.

#[cfg(all(
    any(target_arch = "aarch64", target_arch = "x86_64"),
    target_vendor = "apple"
))]
mod inner {
    use noosphere_storage::NativeStorage;
    use ucan_key_support::ed25519::Ed25519KeyMaterial;

    use crate::key::InsecureKeyStorage;

    // NOTE: This is going to change when we transition to secure key storage
    // This key material type implies insecure storage on disk
    pub type PlatformKeyMaterial = Ed25519KeyMaterial;
    pub type PlatformKeyStorage = InsecureKeyStorage;

    #[cfg(not(feature = "ipfs-storage"))]
    pub type PlatformStorage = NativeStorage;

    #[cfg(feature = "ipfs-storage")]
    use noosphere_ipfs::{IpfsStorage, KuboClient};

    #[cfg(feature = "ipfs-storage")]
    pub type PlatformStorage = IpfsStorage<NativeStorage, KuboClient>;

    #[cfg(test)]
    use anyhow::Result;

    #[cfg(test)]
    use std::path::PathBuf;

    #[cfg(test)]
    use tempfile::TempDir;

    #[cfg(test)]
    pub async fn make_temporary_platform_primitives(
    ) -> Result<(PathBuf, PlatformKeyStorage, (TempDir, TempDir))> {
        let sphere_dir = TempDir::new().unwrap();

        let key_dir = TempDir::new().unwrap();

        let key_storage = InsecureKeyStorage::new(key_dir.path())?;

        Ok((sphere_dir.path().into(), key_storage, (sphere_dir, key_dir)))
    }
}

#[cfg(target_arch = "wasm32")]
mod inner {
    use crate::key::WebCryptoKeyStorage;

    use std::sync::Arc;
    use ucan_key_support::web_crypto::WebCryptoRsaKeyMaterial;

    pub type PlatformKeyMaterial = Arc<WebCryptoRsaKeyMaterial>;
    pub type PlatformKeyStorage = WebCryptoKeyStorage;

    use noosphere_storage::WebStorage;

    #[cfg(feature = "ipfs-storage")]
    pub type PlatformStorage =
        noosphere_ipfs::IpfsStorage<WebStorage, noosphere_ipfs::GatewayClient>;

    #[cfg(not(feature = "ipfs-storage"))]
    pub type PlatformStorage = WebStorage;

    #[cfg(test)]
    use anyhow::Result;

    #[cfg(test)]
    use std::path::PathBuf;

    #[cfg(test)]
    pub async fn make_temporary_platform_primitives() -> Result<(PathBuf, PlatformKeyStorage, ())> {
        let db_name: PathBuf = witty_phrase_generator::WPGen::new()
            .with_words(3)
            .unwrap()
            .into_iter()
            .map(|word| String::from(word))
            .collect();

        let key_storage_name: String = witty_phrase_generator::WPGen::new()
            .with_words(3)
            .unwrap()
            .into_iter()
            .map(|word| String::from(word))
            .collect();

        let key_storage = WebCryptoKeyStorage::new(&key_storage_name).await?;

        Ok((db_name, key_storage, ()))
    }
}

#[cfg(all(
    not(target_arch = "wasm32"),
    not(all(
        any(target_arch = "aarch64", target_arch = "x86_64"),
        target_vendor = "apple"
    ))
))]
mod inner {
    use noosphere_storage::NativeStorage;
    use ucan_key_support::ed25519::Ed25519KeyMaterial;

    use crate::implementation::InsecureKeyStorage;

    pub type PlatformKeyMaterial = Ed25519KeyMaterial;
    pub type PlatformKeyStorage = InsecureKeyStorage;

    #[cfg(not(feature = "ipfs-storage"))]
    pub type PlatformStorage = NativeStorage;

    #[cfg(feature = "ipfs-storage")]
    use noosphere_ipfs::{IpfsStorage, KuboClient};

    #[cfg(feature = "ipfs-storage")]
    pub type PlatformStorage = IpfsStorage<NativeStorage, KuboClient>;

    #[cfg(test)]
    use anyhow::Result;

    #[cfg(test)]
    use std::path::PathBuf;

    #[cfg(test)]
    use tempfile::TempDir;

    #[cfg(test)]
    pub async fn make_temporary_platform_primitives(
    ) -> Result<(PathBuf, PlatformKeyStorage, (TempDir, TempDir))> {
        let sphere_dir = TempDir::new().unwrap();

        let key_dir = TempDir::new().unwrap();

        let key_storage = InsecureKeyStorage::new(key_dir.path())?;

        Ok((sphere_dir.path().into(), key_storage, (sphere_dir, key_dir)))
    }
}

use std::sync::Arc;

pub use inner::*;
use noosphere_sphere::{SphereContext, SphereCursor};
use tokio::sync::Mutex;

use crate::implementation::SphereChannel;

// NOTE: We may someday define the 3rd and 4th terms of this type differently on
// web, where `Arc` and `Mutex` are currently overkill for our needs and may be
// substituted for `Rc` and `RwLock`, respectively.
pub type PlatformSphereContext = SphereCursor<Arc<SphereContext<PlatformStorage>>, PlatformStorage>;
pub type PlatformMutableSphereContext = Arc<Mutex<SphereContext<PlatformStorage>>>;

pub type PlatformSphereChannel =
    SphereChannel<PlatformStorage, PlatformSphereContext, PlatformMutableSphereContext>;
