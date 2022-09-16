use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs::{create_dir_all, File};
use tokio::io::{copy, AsyncRead, AsyncWriteExt};

use super::target::WriteTarget;
use super::WriteTargetConditionalSend;

/// A generalized file system-backed implementation of WriteTarget. It roots
/// all writes to the configured `root`, making it suitable for rendering
/// Noosphere content to a target directory.
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
        // TODO: Need to verify that input path is not climbing up the tree!
        if let Some(parent) = path.parent() {
            create_dir_all(self.root.join(parent)).await?;
        }

        let path = self.root.join(path);
        let mut file = File::create(path).await?;

        copy(&mut contents, &mut file).await?;

        file.flush().await?;

        Ok(())
    }

    async fn symlink(&self, src: &PathBuf, dst: &PathBuf) -> Result<()> {
        // TODO: Need to verify that input path is not climbing up the tree!
        Ok(tokio::fs::symlink(self.root.join(src), self.root.join(dst)).await?)
    }

    async fn spawn<F>(future: F) -> Result<()>
    where
        F: futures::Future<Output = Result<()>> + WriteTargetConditionalSend + 'static,
    {
        tokio::spawn(future).await??;
        Ok(())
    }
}
