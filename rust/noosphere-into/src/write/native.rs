use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
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

impl NativeFs {
    fn assert_relative(path: &Path) -> Result<()> {
        if path.is_absolute() || path.starts_with("..") {
            Err(anyhow!(
                "Only relative sub-paths allowed, but received: {:?}",
                path
            ))
        } else {
            Ok(())
        }
    }
}

#[async_trait]
impl WriteTarget for NativeFs {
    async fn exists(&self, path: &Path) -> Result<bool> {
        Ok(self.root.join(path).exists())
    }

    async fn write<R>(&self, path: &Path, mut contents: R) -> Result<()>
    where
        R: AsyncRead + Unpin + WriteTargetConditionalSend,
    {
        NativeFs::assert_relative(path)?;

        if let Some(parent) = path.parent() {
            create_dir_all(self.root.join(parent)).await?;
        }

        let path = self.root.join(path);
        let mut file = File::create(path).await?;

        copy(&mut contents, &mut file).await?;

        file.flush().await?;

        Ok(())
    }

    async fn symlink(&self, src: &Path, dst: &Path) -> Result<()> {
        NativeFs::assert_relative(src)?;
        NativeFs::assert_relative(dst)?;

        #[cfg(not(windows))]
        let result = tokio::fs::symlink(self.root.join(src), self.root.join(dst)).await?;
        #[cfg(windows)]
        let result = tokio::fs::symlink_file(self.root.join(src), self.root.join(dst)).await?;

        Ok(result)
    }

    async fn spawn<F>(future: F) -> Result<()>
    where
        F: futures::Future<Output = Result<()>> + WriteTargetConditionalSend + 'static,
    {
        tokio::spawn(future).await??;
        Ok(())
    }
}
