use std::str::FromStr;

use crate::native::workspace::Workspace;
use anyhow::{anyhow, Result};
use cid::Cid;
use noosphere::view::Sphere;
use noosphere_storage::{
    db::SphereDb,
    native::{NativeStorageInit, NativeStorageProvider},
};
use tokio::fs;
use ucan::store::UcanJwtStore;

pub async fn sphere_create(owner_key: &str, workspace: &Workspace) -> Result<()> {
    if workspace.expect_local_directories().is_ok() {
        return Err(anyhow!(
            "A sphere is already initialized in {:?}",
            workspace.root_path()
        ));
    }

    workspace.initialize_local_directories().await?;

    let owner_did = workspace
        .get_key_did(owner_key)
        .await
        .map_err(|error| anyhow!("Could not look up the key {:?}:\n{:?}", owner_key, error))?;

    let storage_provider =
        NativeStorageProvider::new(NativeStorageInit::Path(workspace.blocks_path().clone()))?;
    let mut db = SphereDb::new(&storage_provider).await?;

    let (sphere, authorization, mnemonic) = Sphere::try_generate(&owner_did, &mut db).await?;

    let sphere_identity = sphere.try_get_identity().await?;

    let ucan = authorization.resolve_ucan(&db).await?;
    let jwt = ucan.encode()?;

    db.set_version(&sphere_identity, sphere.cid()).await?;
    let jwt_cid = db.write_token(&jwt).await?;

    fs::write(workspace.authorization_path(), &jwt_cid.to_string()).await?;
    fs::write(workspace.key_path(), owner_did).await?;
    fs::write(workspace.identity_path(), sphere_identity).await?;

    println!(
        r#"A new sphere has been created in {:?}
Its identity is {}
Your key {:?} is considered its owner
The owner of a sphere can authorize other keys to write to it

IMPORTANT: Write down the following sequence of words...

{}

...and keep it somewhere safe!
You will be asked to enter them if you ever need to transfer ownership of the sphere to a different key."#,
        workspace.root_path(),
        sphere.try_get_identity().await?,
        owner_key,
        mnemonic
    );

    Ok(())
}

pub async fn sphere_join(
    local_key: &str,
    authorization: Option<String>,
    sphere_did: &str,
    workspace: &Workspace,
) -> Result<()> {
    if workspace.expect_local_directories().is_ok() {
        return Err(anyhow!(
            "A sphere is already initialized in {:?}",
            workspace.root_path()
        ));
    }

    println!("Joining sphere {}...", sphere_did);

    let did = workspace.get_key_did(local_key).await?;

    let cid_string = match authorization {
        None => {
            println!(
                r#"In order to join the sphere, another client must authorize your local key
This is the local key's ID; share it with an authorized client:

  {0}

Hint: if the authorized client is also using the "orb" CLI, you can use this command from the existing workspace to authorize the new key:

  orb auth add {0}
  
Once authorized, you will get a code.
Type or paste the code here and press enter:"#,
                did
            );

            let mut cid_string = String::new();

            std::io::stdin().read_line(&mut cid_string)?;

            cid_string
        }
        Some(cid_string) => cid_string,
    };

    Cid::from_str(cid_string.trim())
        .map_err(|_| anyhow!("Could not parse the authorization identity as a CID"))?;

    workspace.initialize_local_directories().await?;

    fs::write(workspace.identity_path(), sphere_did).await?;
    fs::write(workspace.key_path(), &did).await?;
    fs::write(workspace.authorization_path(), &cid_string).await?;

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
