///! Platform-specific types and bindings

#[cfg(all(
    any(target_arch = "aarch64", target_arch = "x86_64"),
    target_vendor = "apple"
))]
mod inner {
    use noosphere_storage::native::NativeStore;
    use ucan_key_support::ed25519::Ed25519KeyMaterial;

    // NOTE: This is going to change when we transition to secure key storage
    // This key material type implies insecure storage on disk
    pub type PlatformKeyMaterial = Ed25519KeyMaterial;
    pub type PlatformStore = NativeStore;
}

#[cfg(target_arch = "wasm32")]
mod inner {
    use noosphere_storage::web::WebStore;
    use ucan_key_support::WebCryptoRsaKeyMaterial;

    pub type PlatformKeyMaterial = WebCryptoRsaKeyMaterial;
    pub type PlatformStore = WebStore;
}

#[cfg(all(
    not(target_arch = "wasm32"),
    not(all(
        any(target_arch = "aarch64", target_arch = "x86_64"),
        target_vendor = "apple"
    ))
))]
mod inner {
    use noosphere_storage::native::NativeStore;
    use ucan_key_support::ed25519::Ed25519KeyMaterial;

    pub type PlatformKeyMaterial = Ed25519KeyMaterial;
    pub type PlatformStore = NativeStore;
}

pub use inner::*;
