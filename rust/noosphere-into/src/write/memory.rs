use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use super::WriteTargetConditionalSend;
use anyhow::Result;
use async_trait::async_trait;
use futures::Future;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::sync::Mutex;

use super::WriteTarget;

#[derive(Default, Clone)]
pub struct MemoryWriteTarget {
    vfs: Arc<Mutex<BTreeMap<PathBuf, Vec<u8>>>>,
}

impl MemoryWriteTarget {
    pub async fn read(&self, path: &PathBuf) -> Option<Vec<u8>> {
        self.vfs.lock().await.get(path).cloned()
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl WriteTarget for MemoryWriteTarget {
    async fn exists(&self, path: &PathBuf) -> Result<bool> {
        Ok(self.vfs.lock().await.contains_key(path))
    }

    async fn write<R>(&self, path: &PathBuf, contents: &mut R) -> Result<()>
    where
        R: AsyncRead + Unpin + WriteTargetConditionalSend,
    {
        let mut buffer = Vec::new();
        contents.read_to_end(&mut buffer).await?;
        self.vfs.lock().await.insert(path.clone(), buffer);
        Ok(())
    }

    async fn spawn<F>(future: F) -> Result<()>
    where
        F: Future<Output = Result<()>> + WriteTargetConditionalSend + 'static,
    {
        future.await?;
        Ok(())
    }
}
