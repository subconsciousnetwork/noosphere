use crate::native::workspace::Workspace;
use anyhow::{anyhow, Result};
use noosphere::view::Sphere;
use noosphere_storage::{
    db::SphereDb,
    native::{NativeStorageInit, NativeStorageProvider},
};
use tokio::fs;

pub async fn sphere_create(owner_key: &str, working_paths: &Workspace) -> Result<()> {
    if working_paths.expect_local_directories().is_ok() {
        return Err(anyhow!(
            "A sphere is already initialized in {:?}",
            working_paths.root_path()
        ));
    }

    working_paths.initialize_local_directories().await?;

    let owner_did = working_paths
        .get_key_did(owner_key)
        .await
        .map_err(|error| anyhow!("Could not look up the key {:?}:\n{:?}", owner_key, error))?;

    let storage_provider =
        NativeStorageProvider::new(NativeStorageInit::Path(working_paths.blocks_path().clone()))?;
    let mut db = SphereDb::new(&storage_provider).await?;

    let (sphere, ucan, mnemonic) = Sphere::try_generate(&owner_did, &mut db).await?;

    let sphere_identity = sphere.try_get_identity().await?;

    db.set_version(&sphere_identity, sphere.cid()).await?;

    fs::write(working_paths.authorization_path(), ucan.encode()?).await?;
    fs::write(working_paths.key_path(), owner_did).await?;
    fs::write(working_paths.identity_path(), sphere_identity).await?;

    println!(
        r#"A new sphere has been created in {:?}
Its identity is {}
Your key {:?} is considered its owner
The owner of a sphere can authorize other keys to write to it

IMPORTANT: Write down the following sequence of words...

{}

...and keep it somewhere safe!
You will be asked to enter them if you ever need to transfer ownership of the sphere to a different key."#,
        working_paths.root_path(),
        sphere.try_get_identity().await?,
        owner_key,
        mnemonic
    );

    Ok(())
}

pub async fn sphere_join(
    local_key: &str,
    token: Option<String>,
    sphere_did: &str,
    working_paths: &Workspace,
) -> Result<()> {
    if working_paths.expect_local_directories().is_ok() {
        return Err(anyhow!(
            "A sphere is already initialized in {:?}",
            working_paths.root_path()
        ));
    }

    println!("Joining sphere {}...", sphere_did);

    let did = working_paths.get_key_did(local_key).await?;

    let _token = match token {
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

            let mut token = String::new();

            std::io::stdin().read_line(&mut token)?;

            token
        }
        Some(token) => token,
    };

    todo!();
}

pub async fn authorize(_key_did: &str, working_paths: &Workspace) -> Result<()> {
    working_paths.expect_local_directories()?;

    Ok(())
    // TODO: Authorize...
}
