use crate::storage::Storage;
use anyhow::Result;
use async_trait::async_trait;
use noosphere_common::ConditionalSend;
use std::path::Path;

#[cfg(doc)]
use crate::FsBackedStorage;

/// [Storage] that can be opened via [Path] reference.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait OpenStorage: Storage + Sized {
    /// Open [Storage] at `path`.
    async fn open<P: AsRef<Path> + ConditionalSend>(path: P) -> Result<Self>;
}

/// [Storage] that can be deleted via [Path] reference.
/// [FsBackedStorage] types get a blanket implementation.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait DeleteStorage: Storage + Sized {
    /// Delete/clear [Storage] at `path`.
    async fn delete<P: AsRef<Path> + ConditionalSend>(path: P) -> Result<()>;
}

/// [Storage] that can be moved/renamed via [Path] reference.
/// [FsBackedStorage] types get a blanket implementation.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait RenameStorage: Storage + Sized {
    /// Rename/move [Storage] at `path`.
    async fn rename<P: AsRef<Path> + ConditionalSend, Q: AsRef<Path> + ConditionalSend>(
        from: P,
        to: Q,
    ) -> Result<()>;
}
