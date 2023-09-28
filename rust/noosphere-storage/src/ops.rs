use crate::{storage::Storage, IterableStore, Store};
use anyhow::Result;
use async_trait::async_trait;
use noosphere_common::{ConditionalSend, ConditionalSync};
use std::path::Path;
use tokio_stream::StreamExt;

/// [Storage] that can be opened via [Path] reference.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait OpenStorage: Storage + Sized {
    /// Open [Storage] at `path`.
    async fn open<P: AsRef<Path> + ConditionalSend>(path: P) -> Result<Self>;
}

/// [Store] that can take all entries from another store.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait ExtendStore: Store {
    /// Take all entries from `store` are write them to `self`.
    async fn extend<S: IterableStore + ConditionalSync>(&mut self, store: &S) -> Result<()>;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<T> ExtendStore for T
where
    T: Store,
{
    async fn extend<S: IterableStore + ConditionalSync>(&mut self, store: &S) -> Result<()> {
        let mut stream = store.get_all_entries();
        while let Some((key, value)) = stream.try_next().await? {
            self.write(&key, &value).await?;
        }
        Ok(())
    }
}
