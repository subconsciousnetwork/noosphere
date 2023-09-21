use crate::{block::BlockStore, ephemeral::EphemeralStorage, key_value::KeyValueStore};
use anyhow::Result;
use async_trait::async_trait;
use noosphere_common::ConditionalSync;
use std::fmt::Debug;

/// [Storage] is a general trait for composite storage backends. It is often the
/// case that we are able to use a single storage primitive for all forms of
/// storage, but sometimes block storage and generic key/value storage come from
/// different backends. [Storage] provides a composite interface where both
/// cases well accomodated without creating complexity in the signatures of
/// other Noosphere constructs.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait Storage: EphemeralStorage + Clone + ConditionalSync + Debug {
    type BlockStore: BlockStore;
    type KeyValueStore: KeyValueStore;

    /// Get a [BlockStore] where all values stored in it are scoped to the given
    /// name
    async fn get_block_store(&self, name: &str) -> Result<Self::BlockStore>;

    /// Get a [KeyValueStore] where all values stored in it are scoped to the
    /// given name
    async fn get_key_value_store(&self, name: &str) -> Result<Self::KeyValueStore>;
}
