use crate::native::workspace::Workspace;
use anyhow::{anyhow, Result};
use noosphere::view::Sphere;
use noosphere_storage::{
    interface::StorageProvider,
    native::{NativeStorageInit, NativeStorageProvider},
    BLOCK_STORE,
};
use tokio::fs;

pub async fn initialize_sphere(owner_key: &str, working_paths: &Workspace) -> Result<()> {
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
    let mut block_store = storage_provider.get_store(BLOCK_STORE).await?;

    let (sphere, ucan, mnemonic) = Sphere::try_generate(&owner_did, &mut block_store).await?;

    fs::write(working_paths.authorization_path(), ucan.encode()?).await?;
    fs::write(working_paths.key_path(), owner_did).await?;

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

pub async fn join_sphere(
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
                r#"In order to join the sphere, its owner must first authorize your key
This is your key's ID; share it with the owner of the sphere:

  {0}

Hint: if the owner is using the Noosphere CLI, they can use this command from the sphere root directory to authorize your key:

  orb auth add {0}
  
Once authorized, the owner will give you a code.
Type or paste the code here and press enter:"#,
                did
            );

            let mut token = String::new();

            std::io::stdin().read_line(&mut token)?;

            token
        }
        Some(token) => token,
    };
    // working_paths.initialize_local_directories().await?;

    Ok(())
}

pub async fn authorize(_key_did: &str, working_paths: &Workspace) -> Result<()> {
    working_paths.expect_local_directories()?;

    Ok(())
    // TODO: Authorize...
}
