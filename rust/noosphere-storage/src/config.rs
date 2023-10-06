use crate::storage::Storage;
use anyhow::Result;
use async_trait::async_trait;
use noosphere_common::ConditionalSend;
use std::path::Path;

/// Generalized configurations for [ConfigurableStorage].
#[derive(Debug, Clone, Default)]
pub struct StorageConfig {
    /// If set, the size limit in bytes of a memory-based cache.
    pub memory_cache_limit: Option<usize>,
}

/// [Storage] that can be customized via [StorageConfig].
///
/// Configurations are generalized across storage providers,
/// and may have differing underlying semantics.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait ConfigurableStorage: Storage {
    async fn open_with_config<P: AsRef<Path> + ConditionalSend>(
        path: P,
        config: StorageConfig,
    ) -> Result<Self>;
}
