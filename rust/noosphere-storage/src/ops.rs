use crate::storage::Storage;
use anyhow::Result;
use async_trait::async_trait;
use noosphere_common::ConditionalSend;
use std::path::Path;

/// [Storage] that can be opened via [Path] reference.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait OpenStorage: Storage + Sized {
    /// Open [Storage] at `path`.
    async fn open<P: AsRef<Path> + ConditionalSend>(path: P) -> Result<Self>;
}
