///! Platform-specific types and bindings
///! Platforms will vary in capabilities for things like block storage and
///! secure key management. This module lays out the concrete strategies we will
///! use on a per-platform basis.

#[cfg(all(
    any(target_arch = "aarch64", target_arch = "x86_64"),
    target_vendor = "apple"
))]
mod inner {
    use noosphere_storage::native::{NativeStorageProvider, NativeStore};
    use ucan_key_support::ed25519::Ed25519KeyMaterial;

    use crate::key::InsecureKeyStorage;

    // NOTE: This is going to change when we transition to secure key storage
    // This key material type implies insecure storage on disk
    pub type PlatformKeyMaterial = Ed25519KeyMaterial;
    pub type PlatformKeyStorage = InsecureKeyStorage;
    pub type PlatformStore = NativeStore;
    pub type PlatformStorageProvider = NativeStorageProvider;
}

#[cfg(target_arch = "wasm32")]
mod inner {
    use crate::key::WebCryptoKeyStorage;
    use noosphere_storage::web::{WebStorageProvider, WebStore};
    use ucan_key_support::web_crypto::WebCryptoRsaKeyMaterial;

    pub type PlatformKeyMaterial = WebCryptoRsaKeyMaterial;
    pub type PlatformKeyStorage = WebCryptoKeyStorage;
    pub type PlatformStore = WebStore;
    pub type PlatformStorageProvider = WebStorageProvider;
}

#[cfg(all(
    not(target_arch = "wasm32"),
    not(all(
        any(target_arch = "aarch64", target_arch = "x86_64"),
        target_vendor = "apple"
    ))
))]
mod inner {
    use noosphere_storage::native::{NativeStorageProvider, NativeStore};
    use ucan_key_support::ed25519::Ed25519KeyMaterial;

    use crate::key::InsecureKeyStorage;

    pub type PlatformKeyMaterial = Ed25519KeyMaterial;
    pub type PlatformKeyStorage = InsecureKeyStorage;
    pub type PlatformStore = NativeStore;
    pub type PlatformStorageProvider = NativeStorageProvider;
}

pub use inner::*;
