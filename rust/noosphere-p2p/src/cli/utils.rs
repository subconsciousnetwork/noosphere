use anyhow::{anyhow, Result};
use noosphere::authority::restore_ed25519_key;
use noosphere_cli::native::workspace::Workspace;
use tokio::fs;
use ucan_key_support::ed25519::Ed25519KeyMaterial;

/// @TODO currently a private method in Workspace. Use that once migrating
/// to noosphere-cli.
/// Get a mnemonic corresponding to the private portion of a give key by name
pub async fn get_key_mnemonic(workspace: &Workspace, name: &str) -> Result<String> {
    Ok(fs::read_to_string(workspace.keys_path().join(name).with_extension("private")).await?)
}

/// @TODO Integrate into workspace
pub async fn get_address_book(workspace: &Workspace) -> Result<String> {
    let path = workspace
        .root_path()
        .join("address_book")
        .with_extension("toml");
    Ok(fs::read_to_string(path).await?)
}

pub async fn keyname_to_keymaterial(
    workspace: &Workspace,
    keyname: &String,
) -> Result<Ed25519KeyMaterial> {
    //workspace.get_key_mnemonic(keyname.as_str())
    get_key_mnemonic(workspace, keyname.as_str())
        .await
        .and_then(|m| restore_ed25519_key(&m))
        .map_err(|_| anyhow!(format!("Could not find key with name {keyname:?}")))
}
