use crate::storage::Storage;
use anyhow::Result;
use async_trait::async_trait;
use noosphere_common::ConditionalSend;
use std::path::Path;

/// [Storage] that is based on a file system. Implementing [FsBackedStorage]
/// provides blanket implementations for other trait-based [Storage] operations.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait FsBackedStorage: Storage + Sized {
    /// Deletes the storage located at `path` directory. Returns `Ok(())` if
    /// the directory is successfully removed, or if it already does not exist.
    async fn delete<P: AsRef<Path> + ConditionalSend>(path: P) -> Result<()> {
        match std::fs::metadata(path.as_ref()) {
            Ok(_) => std::fs::remove_dir_all(path.as_ref()).map_err(|e| e.into()),
            Err(_) => Ok(()),
        }
    }

    /// Moves the storage located at `from` to the `to` location.
    async fn rename<P: AsRef<Path> + ConditionalSend, Q: AsRef<Path> + ConditionalSend>(
        from: P,
        to: Q,
    ) -> Result<()> {
        std::fs::rename(from, to).map_err(|e| e.into())
    }
}

/// [Storage] that is based on a file system.
#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
pub trait FsBackedStorage: Storage + Sized {
    /// Deletes the storage located at `path` directory. Returns `Ok(())` if
    /// the directory is successfully removed, or if it already does not exist.
    async fn delete<P: AsRef<Path> + ConditionalSend>(path: P) -> Result<()>;

    /// Moves the storage located at `from` to the `to` location.
    async fn rename<P: AsRef<Path> + ConditionalSend, Q: AsRef<Path> + ConditionalSend>(
        from: P,
        to: Q,
    ) -> Result<()>;
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl<T> crate::ops::DeleteStorage for T
where
    T: FsBackedStorage,
{
    async fn delete<P: AsRef<Path> + ConditionalSend>(path: P) -> Result<()> {
        <T as FsBackedStorage>::delete(path).await
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl<T> crate::ops::RenameStorage for T
where
    T: FsBackedStorage,
{
    async fn rename<P: AsRef<Path> + ConditionalSend, Q: AsRef<Path> + ConditionalSend>(
        from: P,
        to: Q,
    ) -> Result<()> {
        <T as FsBackedStorage>::rename(from, to).await
    }
}
