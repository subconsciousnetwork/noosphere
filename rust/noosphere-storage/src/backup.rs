use crate::storage::Storage;
use anyhow::Result;
use async_trait::async_trait;
use noosphere_common::ConditionalSend;
use std::path::{Path, PathBuf};

#[cfg(not(target_arch = "wasm32"))]
use crate::FsBackedStorage;

#[cfg(not(target_arch = "wasm32"))]
fn create_backup_path<P: AsRef<Path>>(path: P) -> Result<PathBuf> {
    use instant::SystemTime;
    use rand::Rng;

    let mut path = path.as_ref().to_owned();
    let timestamp = SystemTime::UNIX_EPOCH
        .elapsed()
        .map_err(|_| anyhow::anyhow!("Could not generate timestamp."))?
        .as_secs();
    let nonce = rand::thread_rng().gen::<u32>();
    path.set_extension(format!("backup.{}-{}", timestamp, nonce));
    Ok(path)
}

/// [Storage] that can be backed up and restored.
/// [FsBackedStorage] types get a blanket implementation.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait BackupStorage: Storage {
    /// Backup [Storage] located at `path`, moving to a backup location.
    async fn backup<P: AsRef<Path> + ConditionalSend>(path: P) -> Result<PathBuf>;
    /// Backup [Storage] at `restore_to`, moving [Storage] from `backup_path` to `restore_to`.
    async fn restore<P: AsRef<Path> + ConditionalSend, Q: AsRef<Path> + ConditionalSend>(
        backup_path: P,
        restore_to: Q,
    ) -> Result<PathBuf>;
    /// List paths to backups for `path`.
    async fn list_backups<P: AsRef<Path> + ConditionalSend>(path: P) -> Result<Vec<PathBuf>>;
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl<T> BackupStorage for T
where
    T: FsBackedStorage,
{
    async fn backup<P: AsRef<Path> + ConditionalSend>(path: P) -> Result<PathBuf> {
        let backup_path = create_backup_path(path.as_ref())?;
        T::rename(path, &backup_path).await?;
        Ok(backup_path)
    }

    async fn restore<P: AsRef<Path> + ConditionalSend, Q: AsRef<Path> + ConditionalSend>(
        backup_path: P,
        restore_to: Q,
    ) -> Result<PathBuf> {
        let restoration_path = restore_to.as_ref().to_owned();
        let original_backup = T::backup(&restoration_path).await?;
        T::rename(backup_path, &restoration_path).await?;
        Ok(original_backup)
    }

    async fn list_backups<P: AsRef<Path> + ConditionalSend>(path: P) -> Result<Vec<PathBuf>> {
        let mut backups = vec![];
        let matcher = format!(
            "{}.backup.",
            path.as_ref()
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Could not stringify path."))?
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Could not stringify path."))?
        );
        let parent_dir = path
            .as_ref()
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Could not find storage parent directory."))?;
        let mut stream = tokio::fs::read_dir(parent_dir).await?;
        while let Ok(Some(entry)) = stream.next_entry().await {
            if let Ok(file_name) = entry.file_name().into_string() {
                if file_name.starts_with(&matcher) {
                    backups.push(entry.path());
                }
            }
        }
        Ok(backups)
    }
}

#[cfg(all(not(target_arch = "wasm32"), test))]
mod test {
    use crate::{OpenStorage, PreferredPlatformStorage, Store};

    use super::*;

    #[tokio::test]
    pub async fn it_can_backup_storages() -> Result<()> {
        noosphere_core_dev::tracing::initialize_tracing(None);

        let temp_dir = tempfile::TempDir::new()?;
        let db_source = temp_dir.path().join("db");

        {
            let storage = PreferredPlatformStorage::open(&db_source).await?;
            let mut store = storage.get_key_value_store("links").await?;
            store.write(b"1", b"1").await?;
        }

        let backup_1 = PreferredPlatformStorage::backup(&db_source).await?;

        {
            let storage = PreferredPlatformStorage::open(&db_source).await?;
            let mut store = storage.get_key_value_store("links").await?;
            assert!(store.read(b"1").await?.is_none(), "Backup is a move");
            store.write(b"2", b"2").await?;
        }

        let backup_2 = PreferredPlatformStorage::backup(&db_source).await?;

        {
            let storage = PreferredPlatformStorage::open(&db_source).await?;
            let mut store = storage.get_key_value_store("links").await?;
            assert!(store.read(b"1").await?.is_none(), "Backup is a move");
            assert!(store.read(b"2").await?.is_none(), "Backup is a move");
            store.write(b"3", b"3").await?;
        }

        let backups = PreferredPlatformStorage::list_backups(&db_source).await?;
        assert_eq!(backups.len(), 2);
        assert!(backups.contains(&backup_1));
        assert!(backups.contains(&backup_2));

        let backup_3 = PreferredPlatformStorage::restore(&backup_1, &db_source).await?;
        {
            let storage = PreferredPlatformStorage::open(&db_source).await?;
            let store = storage.get_key_value_store("links").await?;
            assert_eq!(store.read(b"1").await?.unwrap(), b"1");
            assert!(store.read(b"2").await?.is_none(), "Backup is a move");
            assert!(store.read(b"3").await?.is_none(), "Backup is a move");
        }

        let backups = PreferredPlatformStorage::list_backups(db_source).await?;
        assert_eq!(backups.len(), 2);
        assert!(
            backups.contains(&backup_3),
            "contains backup from restoration."
        );
        assert!(
            !backups.contains(&backup_1),
            "moves backup that was restored."
        );
        assert!(
            backups.contains(&backup_2),
            "contains backups that were untouched."
        );
        Ok(())
    }
}
