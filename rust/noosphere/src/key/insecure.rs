use anyhow::{anyhow, Result};
use async_trait::async_trait;
use noosphere_core::authority::{
    ed25519_key_to_mnemonic, generate_ed25519_key, restore_ed25519_key,
};
use std::path::PathBuf;
use tokio::fs;
use ucan::crypto::KeyMaterial;
use ucan_key_support::ed25519::Ed25519KeyMaterial;

use crate::platform::PlatformKeyMaterial;

use super::KeyStorage;

/// InsecureKeyStorage is a stand-in key storage mechanism to tide us over until
/// we have full-fledged support for secure key storage using TPMs or similar
/// hardware.
///
/// ⚠️ This storage mechanism keeps both public and private key data
/// stored in clear text on disk. User beware!
pub struct InsecureKeyStorage {
    storage_path: PathBuf,
}

impl InsecureKeyStorage {
    pub fn new(global_storage_path: &PathBuf) -> Result<Self> {
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

        tokio::try_join!(
            fs::write(self.private_key_path(name), mnemonic),
            fs::write(self.public_key_path(name), did)
        )?;

        Ok(key_pair)
    }
}
