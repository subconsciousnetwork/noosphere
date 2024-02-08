#[macro_use]
extern crate log;

#[cfg(target_arch = "wasm32")]
pub mod web_crypto;

pub mod ed25519;
pub mod p256;
pub mod rsa;
