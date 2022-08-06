use anyhow::{anyhow, Result};
use std::path::PathBuf;

use crate::gateway::{
    environment::{GatewayConfig, GatewayRoot},
    GatewayKey,
};
use ucan::crypto::KeyMaterial;

pub async fn initialize(directory: &PathBuf, owner_did: &str) -> Result<String> {
    let root = GatewayRoot::at_path(directory);

    info!("Initializing gateway in {}...", root.path().display());

    let mut config = GatewayConfig::from_root(&root);

    if config.get_identity().await?.is_some() {
        return Err(anyhow!(
            "Gateway already intialized; modify config.toml if you need to make changes"
        ));
    }

    let gateway_key = GatewayKey::initialize(&mut config).await?;
    let identity = gateway_key.get_did().await?;

    config.set_identity(&identity).await?;
    config.set_owner_did(owner_did).await?;

    info!(
        "The following configuration was generated: \n{}",
        config.get_raw_contents().await?
    );

    debug!("Gateway initialized in {}", root.path().display());

    Ok(identity)
}
