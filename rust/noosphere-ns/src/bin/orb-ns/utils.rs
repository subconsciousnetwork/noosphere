use anyhow::{anyhow, Result};

use noosphere::key::{InsecureKeyStorage, KeyStorage};
use noosphere_ucan_key_support::ed25519::Ed25519KeyMaterial;
use std::path::PathBuf;

pub async fn get_key_material(
    key_storage: &InsecureKeyStorage,
    key_name: &str,
) -> Result<Ed25519KeyMaterial> {
    if let Some(km) = key_storage.read_key(key_name).await?.take() {
        Ok(km)
    } else {
        Err(anyhow!(
            "No key \"{}\" found in `~/.noosphere/keys/`.",
            key_name
        ))
    }
}

pub fn get_keys_dir() -> Result<PathBuf> {
    Ok(home::home_dir()
        .ok_or_else(|| anyhow!("Could not discover home directory."))?
        .join(".noosphere"))
}
