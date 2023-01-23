#![cfg(not(target_arch = "wasm32"))]
///! IPFS integration for various backend implementations. Currently only Kubo
///! has out-of-the-box support, but integration is based on the generalized
///! [IpfsClient] trait, which opens the possibility for integration with
///! alternative backends in the future. Integration is currently only one-way,
///! but eventually this module will be the entrypoint for pulling blocks out of
///! IPFS backends as well.

mod client;
mod kubo;

pub use client::*;
pub use kubo::*;
