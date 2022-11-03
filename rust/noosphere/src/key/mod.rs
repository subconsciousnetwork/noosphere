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
