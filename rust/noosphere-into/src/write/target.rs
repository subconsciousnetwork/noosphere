use anyhow::Result;
use std::future::Future;
use std::path::Path;

use async_trait::async_trait;
use tokio::io::AsyncRead;

#[cfg(not(target_arch = "wasm32"))]
pub trait WriteTargetConditionalSend: Send {}

#[cfg(not(target_arch = "wasm32"))]
pub trait WriteTargetConditionalSendSync: Send + Sync {}

#[cfg(target_arch = "wasm32")]
pub trait WriteTargetConditionalSend {}

#[cfg(target_arch = "wasm32")]
pub trait WriteTargetConditionalSendSync {}

/// An interface for accessing durable storage. This is used by transformers
/// in this crate to render files from Noosphere content.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait WriteTarget: Clone + WriteTargetConditionalSendSync {
    /// Returns true if a file exists at the provided path
    async fn exists(&self, path: &Path) -> Result<bool>;

    /// Given a path and an [AsyncRead], write the contents of the [AsyncRead]
    /// to the path
    async fn write<R>(&self, path: &Path, contents: R) -> Result<()>
    where
        R: AsyncRead + Unpin + WriteTargetConditionalSend;

    /// Create a symbolic link between the give source path and destination path
    async fn symlink(&self, src: &Path, dst: &Path) -> Result<()>;

    /// Spawn a [Future] in a platform-appropriate fashion and poll it to
    /// completion
    async fn spawn<F>(future: F) -> Result<()>
    where
        F: Future<Output = Result<()>> + WriteTargetConditionalSend + 'static;
}

impl<W> WriteTargetConditionalSendSync for W where W: WriteTarget {}

#[cfg(not(target_arch = "wasm32"))]
impl<S> WriteTargetConditionalSend for S where S: Send {}

#[cfg(target_arch = "wasm32")]
impl<S> WriteTargetConditionalSend for S {}
