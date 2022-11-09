///! Key management is a critical part of working with the Noosphere protocol.
///! This module offers various backing storage mechanisms for key storage,
///! including both insecure and secure options.
mod interface;
pub use interface::*;

#[cfg(not(target_arch = "wasm32"))]
mod insecure;

#[cfg(not(target_arch = "wasm32"))]
pub use insecure::InsecureKeyStorage;

#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
pub use web::WebCryptoKeyStorage;
