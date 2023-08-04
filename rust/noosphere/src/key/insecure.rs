use super::KeyStorage;
use crate::platform::PlatformKeyMaterial;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use noosphere_core::{
    authority::{ed25519_key_to_mnemonic, generate_ed25519_key, restore_ed25519_key},
    data::Did,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use tokio::fs;
use ucan::crypto::KeyMaterial;
use ucan_key_support::ed25519::Ed25519KeyMaterial;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
/// `chmod`-like permission given to private key files on unix systems.
const PRIVATE_KEY_PERMISSIONS: u32 = 0o100600;
#[cfg(unix)]
/// `chmod`-like permission given to public key files on unix systems.
const PUBLIC_KEY_PERMISSIONS: u32 = 0o100644;

/// InsecureKeyStorage is a stand-in key storage mechanism to tide us over until
/// we have full-fledged support for secure key storage using TPMs or similar
/// hardware.
///
/// ⚠️ This storage mechanism keeps both public and private key data
/// stored in clear text on disk. User beware!
#[derive(Clone)]
pub struct InsecureKeyStorage {
    storage_path: PathBuf,
}

impl InsecureKeyStorage {
    pub fn new(global_storage_path: &Path) -> Result<Self> {
        let storage_path = global_storage_path.join("keys");

        std::fs::create_dir_all(&storage_path)?;

        Ok(InsecureKeyStorage { storage_path })
    }

    fn public_key_path(&self, name: &str) -> PathBuf {
        self.storage_path.join(name).with_extension("public")
    }

    fn private_key_path(&self, name: &str) -> PathBuf {
        self.storage_path.join(name).with_extension("private")
    }

    /// The location on disk where all keys are stored
    pub fn storage_path(&self) -> &Path {
        &self.storage_path
    }

    /// Reads all of the "discoverable" keys and returns a BTreeMap of their
    /// credential ID (e.g., their "name") to their DID.
    ///
    /// TODO: This should eventually be a formal part of the [KeyStorage] trait,
    /// but for now we aren't certain if we can depend on the ability to
    /// enumerate keys in general WebAuthn cases.
    ///
    /// See: <https://www.w3.org/TR/webauthn-3/#client-side-discoverable-credential>
    pub async fn get_discoverable_keys(&self) -> Result<BTreeMap<String, Did>> {
        let mut discoverable_keys = BTreeMap::<String, Did>::new();
        let mut directory = fs::read_dir(&self.storage_path).await?;

        while let Some(entry) = directory.next_entry().await? {
            let key_path = entry.path();
            let key_name = key_path.file_stem().map(|stem| stem.to_str());
            let extension = key_path.extension().map(|extension| extension.to_str());

            match (key_name, extension) {
                (Some(Some(key_name)), Some(Some("public"))) => {
                    let did = Did(fs::read_to_string(&key_path).await?);
                    discoverable_keys.insert(key_name.to_string(), did);
                }
                _ => continue,
            };
        }

        Ok(discoverable_keys)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl KeyStorage<Ed25519KeyMaterial> for InsecureKeyStorage {
    async fn require_key(&self, name: &str) -> Result<PlatformKeyMaterial> {
        match self.read_key(name).await? {
            Some(key) => Ok(key),
            None => Err(anyhow!("No key named {} found!", name)),
        }
    }

    async fn read_key(&self, name: &str) -> Result<Option<PlatformKeyMaterial>> {
        let private_key_path = self.private_key_path(name);

        if !private_key_path.exists() {
            return Ok(None);
        }

        let mnemonic = fs::read_to_string(private_key_path).await?;
        let key_pair = restore_ed25519_key(&mnemonic)?;

        Ok(Some(key_pair))
    }

    async fn create_key(&self, name: &str) -> Result<PlatformKeyMaterial> {
        if let Some(key_pair) = self.read_key(name).await? {
            return Ok(key_pair);
        }

        let key_pair = generate_ed25519_key();
        let mnemonic = ed25519_key_to_mnemonic(&key_pair)?;
        let did = key_pair.get_did().await?;

        let private_key_path = self.private_key_path(name);
        let public_key_path = self.public_key_path(name);

        tokio::try_join!(
            fs::write(&private_key_path, mnemonic),
            fs::write(&public_key_path, did)
        )?;

        #[cfg(unix)]
        tokio::try_join!(
            fs::set_permissions(
                &private_key_path,
                std::fs::Permissions::from_mode(PRIVATE_KEY_PERMISSIONS)
            ),
            fs::set_permissions(
                &public_key_path,
                std::fs::Permissions::from_mode(PUBLIC_KEY_PERMISSIONS)
            ),
        )?;

        Ok(key_pair)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::KeyStorage;
    use tempfile::TempDir;
    use tokio::fs;
    use ucan::crypto::KeyMaterial;

    #[tokio::test]
    async fn it_can_create_and_read_a_key() {
        let temp_dir = TempDir::new().unwrap();

        let created_key = {
            let key_storage = InsecureKeyStorage::new(temp_dir.path()).unwrap();
            key_storage.create_key("foo").await.unwrap()
        };

        let retrieved_key = {
            let key_storage = InsecureKeyStorage::new(temp_dir.path()).unwrap();
            key_storage.require_key("foo").await.unwrap()
        };

        assert_eq!(
            created_key.get_did().await.unwrap(),
            retrieved_key.get_did().await.unwrap()
        )
    }

    #[tokio::test]
    async fn it_lists_all_the_created_keys() {
        let temp_dir = TempDir::new().unwrap();

        {
            let key_storage = InsecureKeyStorage::new(temp_dir.path()).unwrap();
            for i in [1, 2, 3, 4, 5] {
                key_storage.create_key(&format!("key{}", i)).await.unwrap();
            }
        }

        {
            let key_storage = InsecureKeyStorage::new(temp_dir.path()).unwrap();
            let keys = key_storage.get_discoverable_keys().await.unwrap();

            for i in [1, 2, 3, 4, 5] {
                assert!(keys.contains_key(&format!("key{}", i)));
            }
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn it_sets_permissions_on_keys() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let key_storage = InsecureKeyStorage::new(temp_dir.path()).unwrap();
        key_storage.create_key("foo").await?;

        let private_key_path = temp_dir
            .path()
            .join("keys")
            .join("foo")
            .with_extension("private");
        let public_key_path = temp_dir
            .path()
            .join("keys")
            .join("foo")
            .with_extension("public");

        let private_key = fs::File::open(private_key_path).await?;
        assert_eq!(
            private_key.metadata().await?.permissions().mode(),
            PRIVATE_KEY_PERMISSIONS
        );
        let public_key = fs::File::open(public_key_path).await?;
        assert_eq!(
            public_key.metadata().await?.permissions().mode(),
            PUBLIC_KEY_PERMISSIONS
        );
        Ok(())
    }
}
