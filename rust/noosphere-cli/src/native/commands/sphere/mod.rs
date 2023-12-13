//! Concrete implementations of the various subcommands of the sphere command

mod auth;
mod config;
mod follow;
mod history;
mod render;
mod save;
mod status;
mod sync;

pub use auth::*;
pub use config::*;
pub use follow::*;
pub use history::*;
pub use render::*;
pub use save::*;
pub use status::*;
pub use sync::*;

use std::{str::FromStr, sync::Arc};

use crate::native::{paths::SpherePaths, workspace::Workspace};
use anyhow::{anyhow, Result};
use cid::Cid;
use noosphere::{
    key::KeyStorage,
    sphere::{SphereContextBuilder, SphereContextBuilderArtifacts},
};
use noosphere_core::{authority::Authorization, data::Did};
use noosphere_core::{
    context::{HasMutableSphereContext, SphereContext, SphereSync},
    data::Mnemonic,
};

use tokio::sync::Mutex;
use ucan::crypto::KeyMaterial;
use url::Url;

/// Create a sphere, assigning authority to modify it to the given key
/// (specified by nickname)
pub async fn sphere_create(owner_key: &str, workspace: &mut Workspace) -> Result<(Did, Mnemonic)> {
    workspace.ensure_sphere_uninitialized()?;

    let sphere_paths = SpherePaths::new(workspace.working_directory());

    let sphere_context_artifacts = SphereContextBuilder::default()
        .create_sphere()
        .at_storage_path(sphere_paths.root())
        .reading_keys_from(workspace.key_storage().clone())
        .using_key(owner_key)
        .build()
        .await?;

    let mnemonic = sphere_context_artifacts.require_mnemonic()?.to_string();
    let sphere_context: SphereContext<_> = sphere_context_artifacts.into();
    let sphere_identity = sphere_context.identity();

    info!(
        r#"A new sphere has been created in {:?}
Its identity is {}
Your key {:?} is considered its owner
The owner of a sphere can authorize other keys to write to it

IMPORTANT: Write down the following sequence of words...

{}

...and keep it somewhere safe!
You will be asked to enter them if you ever need to transfer ownership of the sphere to a different key."#,
        sphere_paths.root(),
        sphere_identity,
        owner_key,
        mnemonic
    );

    workspace.initialize(sphere_paths).await?;

    Ok((sphere_identity.clone(), mnemonic.into()))
}

/// Join an existing sphere
pub async fn sphere_join(
    local_key: &str,
    authorization: Option<String>,
    sphere_identity: &Did,
    gateway_url: &Url,
    render_depth: Option<u32>,
    workspace: &mut Workspace,
) -> Result<()> {
    workspace.ensure_sphere_uninitialized()?;
    info!("Joining sphere {sphere_identity}...");

    let did = {
        let local_key = workspace.key_storage().require_key(local_key).await?;
        local_key.get_did().await?
    };

    let cid_string = match authorization {
        None => {
            info!(
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

    let sphere_paths = SpherePaths::new(workspace.working_directory());

    {
        let mut sphere_context = Arc::new(Mutex::new(
            match SphereContextBuilder::default()
                .join_sphere(sphere_identity)
                .at_storage_path(sphere_paths.root())
                .reading_keys_from(workspace.key_storage().clone())
                .using_key(local_key)
                .authorized_by(Some(&Authorization::Cid(cid)))
                .build()
                .await?
            {
                SphereContextBuilderArtifacts::SphereCreated { context, .. } => context,
                SphereContextBuilderArtifacts::SphereOpened(context) => context,
            },
        ));

        sphere_context
            .sphere_context_mut()
            .await?
            .configure_gateway_url(Some(gateway_url))
            .await?;
        sphere_context.sync().await?;
    }

    workspace.initialize(sphere_paths).await?;
    workspace.render(render_depth, true).await?;

    // TODO(#103): Recovery path if the auth needs to change for some reason

    info!(
        r#"The authorization has been saved. You should be able to sync:

  orb sphere sync
  
Happy pondering!"#
    );

    Ok(())
}
