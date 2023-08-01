use std::{str::FromStr, sync::Arc};

use crate::native::{paths::SpherePaths, workspace::Workspace};
use anyhow::{anyhow, Result};
use cid::Cid;
use noosphere::{
    key::KeyStorage,
    sphere::{SphereContextBuilder, SphereContextBuilderArtifacts},
};
use noosphere_core::{authority::Authorization, data::Did};
use noosphere_sphere::{
    HasMutableSphereContext, SphereContext, SpherePetnameWrite, SphereSync, SyncRecovery,
};

use tokio::sync::Mutex;
use ucan::crypto::KeyMaterial;
use url::Url;

use super::save::save;

pub async fn sphere_create(owner_key: &str, workspace: &mut Workspace) -> Result<()> {
    workspace.ensure_sphere_uninitialized()?;

    let sphere_paths = SpherePaths::intialize(workspace.working_directory()).await?;

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

    workspace.initialize(sphere_paths)?;

    Ok(())
}

pub async fn sphere_join(
    local_key: &str,
    authorization: Option<String>,
    sphere_identity: &Did,
    gateway_url: &Url,
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

    let sphere_paths = SpherePaths::intialize(workspace.working_directory()).await?;

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
        sphere_context.sync(SyncRecovery::None).await?;
    }

    workspace.initialize(sphere_paths)?;
    workspace.render().await?;
    // TODO(#103): Recovery path if the auth needs to change for some reason

    info!(
        r#"The authorization has been saved. You should be able to sync:

  orb sphere sync
  
Happy pondering!"#
    );

    Ok(())
}

pub async fn sphere_follow(
    name: Option<String>,
    did: Option<Did>,
    workspace: &Workspace,
) -> Result<()> {
    workspace.ensure_sphere_initialized()?;

    let sphere_identity = workspace.sphere_identity().await?;
    let (closest_sphere_identity, closest_sphere_version, link_record) = workspace
        .resolve_closest_sphere(None)
        .await?
        .ok_or_else(|| anyhow!("Couldn't resolve local sphere and/or version"))?;
    let closest_sphere_is_root_sphere = closest_sphere_identity == sphere_identity;

    let did = match did {
        Some(did) => did,
        None if !closest_sphere_is_root_sphere => closest_sphere_identity.clone(),
        _ => {
            info!(
                r#"Type or paste the sphere ID (e.g., did:key:...) you want to follow and press enter:"#
            );

            let mut did_string = String::new();
            std::io::stdin().read_line(&mut did_string)?;
            // TODO: Validate this is a supported DID method and give feedback if not
            did_string.trim().into()
        }
    };

    info!("Following Sphere {did}");

    let name = match name {
        Some(name) => name,
        None => {
            let name = if closest_sphere_is_root_sphere {
                None
            } else {
                workspace
                    .resolve_profile_nickname(&closest_sphere_identity, &closest_sphere_version)
                    .await?
            };

            if let Some(name) = name {
                name
            } else {
                info!(r#"Type a nickname for the sphere and press enter:"#);
                let mut name = String::new();
                std::io::stdin().read_line(&mut name)?;
                name.trim().to_owned()
            }
        }
    };

    info!("Assigning petname {name}");

    // TODO: Attempt to automatically discover the locally-loaded [LinkRecord]
    // for the sphere noting that this is only possible when following a FoaF
    // that has been locally rendered
    let mut sphere_context = workspace.sphere_context().await?;

    sphere_context.set_petname(&name, Some(did.clone())).await?;

    if let Some(link_record) = link_record {
        trace!("A link record was found for {did}: {}", link_record);
        sphere_context.save(None).await?;
        sphere_context
            .set_petname_record(&name, &link_record)
            .await?;
    } else {
        info!(
            r#"You are following '@{name}' but a link record must be resolved before you can see their sphere.
        
In order to get a link record, you must sync with a gateway at least twice (ideally a few seconds apart):

  orb sphere sync
  
After the first sync, the gateway will try to resolve the link record for you, and you'll receive it upon a future sync."#
        );
    }

    save(workspace).await?;

    Ok(())
}

pub async fn sphere_unfollow(name: String, workspace: &Workspace) -> Result<()> {
    workspace.ensure_sphere_initialized()?;

    workspace
        .sphere_context()
        .await?
        .set_petname(&name, None)
        .await?;

    save(workspace).await?;

    Ok(())
}
