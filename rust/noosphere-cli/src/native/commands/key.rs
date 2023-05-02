use anyhow::Result;
use noosphere::key::KeyStorage;
use serde_json::json;
use ucan::crypto::KeyMaterial;

use crate::native::workspace::Workspace;

pub static SERVICE_NAME: &str = "noosphere";

pub async fn key_create(name: &str, workspace: &Workspace) -> Result<()> {
    let key = workspace.key_storage().create_key(name).await?;
    let did = key.get_did().await?;

    info!(
        "Created key {:?} in {:?}",
        name,
        workspace.key_storage().storage_path()
    );
    info!("Public identity {did}");

    Ok(())
}

pub async fn key_list(as_json: bool, workspace: &Workspace) -> Result<()> {
    let keys = workspace.key_storage().get_discoverable_keys().await?;
    let max_name_length = keys
        .iter()
        .fold(7, |length, (key_name, _)| key_name.len().max(length));

    if as_json {
        info!("{}", serde_json::to_string_pretty(&json!(keys))?);
    } else {
        info!("{:1$}  IDENTITY", "NAME", max_name_length);
        for (name, did) in keys {
            info!("{name:max_name_length$}  {did}");
        }
    }

    Ok(())
}
