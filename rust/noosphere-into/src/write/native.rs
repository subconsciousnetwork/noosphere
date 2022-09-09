use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::AsyncRead;
use tokio::io::AsyncWriteExt;

use super::target::WriteTarget;
use super::WriteTargetConditionalSend;

#[derive(Clone)]
pub struct NativeFs {
    pub root: PathBuf,
}

impl NativeFs {}

#[async_trait]
impl WriteTarget for NativeFs {
    async fn exists(&self, path: &PathBuf) -> Result<bool> {
        Ok(self.root.join(path).exists())
    }

    async fn write<R>(&self, path: &PathBuf, mut contents: &mut R) -> Result<()>
    where
        R: AsyncRead + Unpin + WriteTargetConditionalSend,
    {
        let path = self.root.join(path);
        let mut file = File::create(path).await?;

        tokio::io::copy(&mut contents, &mut file).await?;
        file.flush().await?;

        Ok(())
    }

    async fn spawn<F>(future: F) -> Result<()>
    where
        F: futures::Future<Output = Result<()>> + WriteTargetConditionalSend + 'static,
    {
        tokio::spawn(future).await??;
        Ok(())
    }
}
