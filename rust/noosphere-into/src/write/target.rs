use anyhow::Result;
use std::future::Future;
use std::path::{Path, PathBuf};

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

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait WriteTarget: Clone + WriteTargetConditionalSendSync {
    async fn exists(&self, path: &PathBuf) -> Result<bool>;
    async fn write<R>(&self, path: &PathBuf, contents: &mut R) -> Result<()>
    where
        R: AsyncRead + Unpin + WriteTargetConditionalSend;

    async fn spawn<F>(future: F) -> Result<()>
    where
        F: Future<Output = Result<()>> + WriteTargetConditionalSend + 'static;
}

impl<W> WriteTargetConditionalSendSync for W where W: WriteTarget {}

#[cfg(not(target_arch = "wasm32"))]
impl<S> WriteTargetConditionalSend for S where S: Send {}

#[cfg(target_arch = "wasm32")]
impl<S> WriteTargetConditionalSend for S {}
