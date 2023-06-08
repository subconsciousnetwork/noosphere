//! IPFS integration for various backend implementations.
//! Provides the generalized [IpfsClient] trait, and implementations
//! for Kubo's HTTP RPC API, and a more limited IPFS HTTP Gateway.
mod gateway;
pub use gateway::GatewayClient;

#[cfg(not(target_arch = "wasm32"))]
mod kubo;
#[cfg(not(target_arch = "wasm32"))]
pub use kubo::KuboClient;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use cid::Cid;
use std::fmt::Debug;
use tokio::io::AsyncRead;

#[cfg(not(target_arch = "wasm32"))]
pub trait IpfsClientAsyncReadSendSync: AsyncRead + Send + Sync + 'static {}
#[cfg(not(target_arch = "wasm32"))]
impl<S> IpfsClientAsyncReadSendSync for S where S: AsyncRead + Send + Sync + 'static {}

#[cfg(target_arch = "wasm32")]
pub trait IpfsClientAsyncReadSendSync: AsyncRead {}
#[cfg(target_arch = "wasm32")]
impl<S> IpfsClientAsyncReadSendSync for S where S: AsyncRead {}

/// A generic interface for interacting with an IPFS-like backend where it may
/// be desirable to syndicate sphere data to. Although the interface was
/// designed after a small subset of the capabilities of IPFS Kubo, it is
/// intended to be general enough to apply to other IPFS implementations.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait IpfsClient: Clone + Debug {
    /// Returns true if the block (referenced by [Cid]) is pinned by the IPFS
    /// server
    async fn block_is_pinned(&self, cid: &Cid) -> Result<bool>;

    /// Returns a string that represents the identity (for example, a
    /// base64-encoded public key) of a node. This node is used to track
    /// syndication progress over time, so it should ideally be stable for a
    /// given server as the client interacts with it over time
    async fn server_identity(&self) -> Result<String>;

    /// Given some CAR bytes, syndicate that CAR to the IPFS server. Callers
    /// expect the roots in the CAR to be explicitly pinned, and for their
    /// descendents to be pinned by association.
    async fn syndicate_blocks<R>(&self, car: R) -> Result<()>
    where
        R: IpfsClientAsyncReadSendSync;

    /// Returns the associated block (referenced by [Cid]) if found.
    async fn get_block(&self, cid: &Cid) -> Result<Option<Vec<u8>>>;

    /// Places the associated block with cid on the corresponding backend.
    async fn put_block(&mut self, cid: &Cid, block: &[u8]) -> Result<()>;

    /// Same as `get_block`, but turns a `None` value into an error.
    async fn require_block(&self, cid: &Cid) -> Result<Vec<u8>> {
        match self.get_block(cid).await? {
            Some(block) => Ok(block),
            None => Err(anyhow!("No block found for CID {}", cid)),
        }
    }
}
