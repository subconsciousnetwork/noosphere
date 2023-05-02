use std::str::FromStr;

use crate::native::workspace::Workspace;
use anyhow::{anyhow, Result};
use cid::Cid;
use noosphere::{key::KeyStorage, sphere::SphereContextBuilder};
use noosphere_core::{authority::Authorization, data::Did};
use noosphere_sphere::SphereContext;

use ucan::crypto::KeyMaterial;

pub async fn sphere_create(owner_key: &str, workspace: &Workspace) -> Result<()> {
    workspace.ensure_sphere_uninitialized()?;

    let sphere_context_artifacts = SphereContextBuilder::default()
        .create_sphere()
        .at_storage_path(workspace.root_directory())
        .reading_keys_from(workspace.key_storage().clone())
        .using_key(owner_key)
        .build()
        .await?;

    let mnemonic = sphere_context_artifacts.require_mnemonic()?.to_string();
    let sphere_context: SphereContext<_, _> = sphere_context_artifacts.into();
    let sphere_identity = sphere_context.identity();

    println!(
        r#"A new sphere has been created in {:?}
Its identity is {}
Your key {:?} is considered its owner
The owner of a sphere can authorize other keys to write to it

IMPORTANT: Write down the following sequence of words...

{}

...and keep it somewhere safe!
You will be asked to enter them if you ever need to transfer ownership of the sphere to a different key."#,
        workspace.root_directory(),
        sphere_identity,
        owner_key,
        mnemonic
    );

    Ok(())
}

pub async fn sphere_join(
    local_key: &str,
    authorization: Option<String>,
    sphere_identity: &Did,
    workspace: &Workspace,
) -> Result<()> {
    workspace.ensure_sphere_uninitialized()?;
    println!("Joining sphere {sphere_identity}...");

    let did = {
        let local_key = workspace.key_storage().require_key(local_key).await?;
        local_key.get_did().await?
    };

    let cid_string = match authorization {
        None => {
            println!(
                r#"In order to join the sphere, another client must authorize your local key
This is the local key's ID; share it with an authorized client:

  {did}

Hint: if the authorized client is also using the "orb" CLI, you can use this command from the existing workspace to authorize the new key:

  orb auth add {did}
  
Once authorized, you will get a code.
Type or paste the code here and press enter:"#
            );

            let mut cid_string = String::new();

            std::io::stdin().read_line(&mut cid_string)?;

            cid_string
        }
        Some(cid_string) => cid_string,
    };

    let cid = Cid::from_str(cid_string.trim())
        .map_err(|_| anyhow!("Could not parse the authorization identity as a CID"))?;

    SphereContextBuilder::default()
        .join_sphere(sphere_identity)
        .at_storage_path(workspace.root_directory())
        .reading_keys_from(workspace.key_storage().clone())
        .using_key(local_key)
        .authorized_by(Some(&Authorization::Cid(cid)))
        .build()
        .await?;

    // TODO(#103): Recovery path if the auth needs to change for some reason

    println!(
        r#"The authorization has been saved.
Make sure that you have configured the gateway's URL:

  orb config set gateway-url <URL>
  
And then you should be able to sync:

  orb sync
  
Happy pondering!"#
    );

    Ok(())
}
