use anyhow::Result;
use async_trait::async_trait;
use noosphere_common::ConditionalSend;

/// [Space] is a general trait for a storage provider to provide
/// a the size on disk, used to calculate space amplification.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait Space: ConditionalSend {
    /// Get the underlying (e.g. disk) space usage of a storage provider.
    async fn get_space_usage(&self) -> Result<u64>;
}

#[allow(unused)]
#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn get_dir_size(path: impl Into<std::path::PathBuf>) -> Result<u64> {
    use std::{future::Future, pin::Pin};

    let path = path.into();
    fn dir_size(mut dir: tokio::fs::ReadDir) -> Pin<Box<dyn Future<Output = Result<u64>> + Send>> {
        Box::pin(async move {
            let mut total_size = 0;
            while let Some(entry) = dir.next_entry().await? {
                let size = match entry.metadata().await? {
                    data if data.is_dir() => {
                        dir_size(tokio::fs::read_dir(entry.path()).await?).await?
                    }
                    data => data.len(),
                };
                total_size += size;
            }
            Ok(total_size)
        })
    }

    dir_size(tokio::fs::read_dir(path).await?).await
}
