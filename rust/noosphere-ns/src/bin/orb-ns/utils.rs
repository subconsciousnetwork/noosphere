use anyhow::{anyhow, Result};

use noosphere::key::{InsecureKeyStorage, KeyStorage};
use std::future::Future;
use std::path::PathBuf;
use ucan_key_support::ed25519::Ed25519KeyMaterial;

/// Runs a future that may await into an `Err`, or otherwise is a long-running,
/// empty future. That future will be held until a Ctrl+C signal
/// is received.
pub async fn run_until_abort(future: impl Future<Output = Result<()>>) -> Result<()> {
    // Allow aborting (ctrl+c) during the initial run,
    // and after (when we want to wait exclusively for ctrl+c signal)
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        result = future => { result?; }
    };
    Ok(())
}

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
