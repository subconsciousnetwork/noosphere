use anyhow::{anyhow, Result};
use serde_json::json;
use tokio::fs;
use ucan::crypto::KeyMaterial;

use noosphere::authority::{ed25519_key_to_mnemonic, generate_ed25519_key};

use crate::native::workspace::Workspace;

pub static SERVICE_NAME: &str = "noosphere";

pub async fn key_create(name: String, working_paths: &Workspace) -> Result<()> {
    working_paths.initialize_global_directories().await?;

    let key_base_path = working_paths.keys_path().join(&name);
    let private_key_path = key_base_path.with_extension("private");

    if private_key_path.exists() {
        return Err(anyhow!("A key called {:?} already exists!", name));
    }

    let did_path = key_base_path.with_extension("public");

    let key_pair = generate_ed25519_key();
    let did = key_pair.get_did().await?;

    let mnemonic = ed25519_key_to_mnemonic(&key_pair)?;

    tokio::try_join!(
        fs::write(private_key_path, mnemonic),
        fs::write(did_path, &did),
    )?;

    println!("Created key {:?} in {:?}", name, working_paths.keys_path());
    println!("Public identity {}", did);

    Ok(())
}

pub async fn key_list(as_json: bool, working_paths: &Workspace) -> Result<()> {
    if let Err(error) = working_paths.expect_global_directories() {
        return Err(anyhow!(
            "{:?}\nTip: you may need to create a key first",
            error
        ));
    }

    let keys = working_paths.get_all_keys().await?;
    let max_name_length = keys
        .iter()
        .fold(7, |length, (key_name, _)| key_name.len().max(length));

    if as_json {
        println!("{}", serde_json::to_string_pretty(&json!(keys))?);
    } else {
        println!("{:1$}  IDENTITY", "NAME", max_name_length);
        for (name, did) in keys {
            println!("{:1$}  {did}", name, max_name_length);
        }
    }

    Ok(())
}
