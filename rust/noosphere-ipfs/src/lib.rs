///! IPFS integration for various backend implementations.
///! Provides the generalized [IpfsClient] trait, and implementations
///! for Kubo's HTTP RPC API, and a more limited IPFS HTTP Gateway.
mod client;
mod gateway;

pub use client::{IpfsClient, IpfsClientAsyncReadSendSync};
pub use gateway::GatewayClient;

#[cfg(not(target_arch = "wasm32"))]
mod kubo;
#[cfg(not(target_arch = "wasm32"))]
pub use kubo::KuboClient;
