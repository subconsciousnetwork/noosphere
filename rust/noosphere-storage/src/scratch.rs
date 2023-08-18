use crate::key_value::KeyValueStore;
use anyhow::Result;
use async_trait::async_trait;

#[cfg(not(target_arch = "wasm32"))]
pub trait ScratchSendSync: Send + Sync {}

#[cfg(not(target_arch = "wasm32"))]
impl<T> ScratchSendSync for T where T: Send + Sync {}

#[cfg(target_arch = "wasm32")]
pub trait ScratchSendSync {}

#[cfg(target_arch = "wasm32")]
impl<T> ScratchSendSync for T {}

/// [Scratch] is a general trait for a storage provider to provide
/// a temporary, isolated [KeyValueStore] that does not persist after dropping.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait Scratch: ScratchSendSync {
    type ScratchStore: KeyValueStore;

    async fn get_scratch_store(&self) -> Result<Self::ScratchStore>;
}
