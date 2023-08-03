use std::collections::BTreeMap;

use crate::native::{commands::sphere::save, workspace::Workspace};
use anyhow::{anyhow, Result};
use noosphere_core::data::Did;
use noosphere_sphere::{
    HasMutableSphereContext, SpherePetnameRead, SpherePetnameWrite, SphereWalker,
};
use serde_json::{json, Value};
use tokio_stream::StreamExt;

/// Add a peer to your address book by assigning their sphere ID to a nickname
pub async fn follow_add(
    name: Option<String>,
    did: Option<Did>,
    workspace: &Workspace,
) -> Result<()> {
    workspace.ensure_sphere_initialized()?;

    let sphere_identity = workspace.sphere_identity().await?;

    let closest_sphere_details = workspace.describe_closest_sphere(None).await?;

    let did = match (did, &closest_sphere_details) {
        // A DID is specified by caller, use it
        (Some(did), _) => did,
        // No DID specified, but we might be able to infer it from the current directory
        (None, Some((closest_sphere_identity, _, _)))
            if closest_sphere_identity != &sphere_identity =>
        {
            closest_sphere_identity.clone()
        }
        // No DID specified, and we cannot infer it
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
        // A name is specified by caller, use it
        Some(name) => name,
        // No name is specified....
        None => {
            let name = match &closest_sphere_details {
                // ...but we can try to infer a good default from a Subconscious profile
                Some((closest_sphere_identity, closest_sphere_version, _))
                    if closest_sphere_identity == &did =>
                {
                    workspace
                        .read_subconscious_flavor_profile_nickname(
                            closest_sphere_identity,
                            closest_sphere_version,
                        )
                        .await?
                }
                // ...and we cannot infer it
                _ => None,
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

    info!("Assigning nickname '@{name}' to sphere {did}...");

    // TODO: Attempt to automatically discover the locally-loaded [LinkRecord]
    // for the sphere noting that this is only possible when following a FoaF
    // that has been locally rendered
    let mut sphere_context = workspace.sphere_context().await?;

    sphere_context.set_petname(&name, Some(did.clone())).await?;

    match closest_sphere_details {
        Some((closest_sphere_identity, _, Some(link_record))) if closest_sphere_identity == did => {
            trace!("A link record was found for {did}: {}", link_record);
            sphere_context.save(None).await?;
            sphere_context
                .set_petname_record(&name, &link_record)
                .await?;
        }
        _ => {
            info!(
                r#"You are following '@{name}' but a link record must be resolved before you can see their sphere.
        
In order to get a link record, you must sync with a gateway at least twice (ideally a few seconds apart):

  orb sphere sync
  
After the first sync, the gateway will try to resolve the link record for you, and you'll receive it upon a future sync."#
            );
        }
    }

    save(None, workspace).await?;

    Ok(())
}

/// Remove a peer to your address book either by nickname or by [Did] (but not both); if
/// the removal is by [Did], all petnames assigned to that [Did] will be removed.
pub async fn follow_remove(
    by_name: Option<String>,
    mut by_sphere_id: Option<Did>,
    workspace: &Workspace,
) -> Result<()> {
    workspace.ensure_sphere_initialized()?;

    if by_name.is_some() && by_sphere_id.is_some() {
        return Err(anyhow!(
            "Can only unfollow by name or by sphere ID, not both"
        ));
    }

    match (&by_name, &by_sphere_id) {
        (Some(_), Some(_)) => {
            return Err(anyhow!(
                "Can only unfollow by name or by sphere ID, not both"
            ));
        }
        (None, None) => {
            let (closest_sphere_identity, _, _) = workspace
                .describe_closest_sphere(None)
                .await?
                .ok_or_else(|| anyhow!("Couldn't resolve local sphere and/or version"))?;

            let local_sphere_identity = workspace.sphere_identity().await?;

            if closest_sphere_identity == local_sphere_identity {
                return Err(anyhow!("Unable to determine which sphere to unfollow"));
            }

            by_sphere_id = Some(closest_sphere_identity);
        }
        _ => (),
    };

    let mut sphere_context = workspace.sphere_context().await?;

    match (by_name, by_sphere_id) {
        (Some(name), _) => {
            info!("Unfollowing '@{}'...", name);
            sphere_context.set_petname(&name, None).await?;
        }
        (_, Some(sphere_identity)) => {
            let names = sphere_context
                .get_assigned_petnames(&sphere_identity)
                .await?;

            if names.is_empty() {
                return Err(anyhow!("Not following sphere {sphere_identity}"));
            }

            info!(
                "There are {} nicknames assigned to sphere {sphere_identity}",
                names.len()
            );

            for name in names {
                info!("Unfollowing '@{name}'...");
                sphere_context.set_petname(&name, None).await?;
            }
        }
        _ => (),
    };

    save(None, workspace).await?;

    Ok(())
}

/// Rename an entry in the sphere's address book to something new
pub async fn follow_rename(from: String, to: Option<String>, workspace: &Workspace) -> Result<()> {
    workspace.ensure_sphere_initialized()?;

    let mut sphere_context = workspace.sphere_context().await?;

    let current_peer_identity = if let Some(did) = sphere_context.get_petname(&from).await? {
        did
    } else {
        return Err(anyhow!(
            "The nickname '@{from}' is not assigned to any sphere you are following"
        ));
    };

    let to = if let Some(to) = to {
        to
    } else {
        let mut name = String::new();
        std::io::stdin().read_line(&mut name)?;
        name.trim().to_owned()
    };

    if to.is_empty() {
        return Err(anyhow!(
            "The new name was empty or not specified; not renaming.."
        ));
    }

    info!("Renaming '@{from}' to '@{to}'...");

    sphere_context.set_petname(&from, None).await?;
    sphere_context
        .set_petname(&to, Some(current_peer_identity))
        .await?;

    if let Some(current_peer_link_record) = sphere_context.get_petname_record(&from).await? {
        debug!("'@{from}' already had a link record, moving it over to '@{to}'");

        sphere_context.save(None).await?;
        sphere_context
            .set_petname_record(&to, &current_peer_link_record)
            .await?;
    }

    save(None, workspace).await?;

    Ok(())
}

/// List all the entries in the address book, optionally in JSON format
pub async fn follow_list(as_json: bool, workspace: &Workspace) -> Result<()> {
    workspace.ensure_sphere_initialized()?;

    let sphere_context = workspace.sphere_context().await?;

    let walker = SphereWalker::from(&sphere_context);

    let petname_stream = walker.petname_stream();

    tokio::pin!(petname_stream);

    let mut rows = BTreeMap::new();
    let mut max_name_length = 7usize;
    let mut max_did_length = 7usize;

    let db = workspace.db().await?;

    while let Some((petname, identity)) = petname_stream.try_next().await? {
        max_name_length = max_name_length.max(petname.len());
        max_did_length = max_did_length.max(identity.did.len());

        let version = if let Some(link_record) = identity.link_record(&db).await {
            link_record.get_link()
        } else {
            None
        };

        rows.insert(petname, (identity.did, version));
    }

    if as_json {
        let value: Value = rows
            .into_iter()
            .map(|(name, (did, version))| {
                (
                    name,
                    json!({
                        "sphere": did.to_string(),
                        "version": version.map(|cid| cid.to_string())
                    }),
                )
            })
            .collect();
        info!("{}", serde_json::to_string_pretty(&value)?);
    } else if rows.is_empty() {
        info!("Not currently following any spheres")
    } else {
        info!(
            "{:max_name_length$}  {:max_did_length$}  VERSION",
            "NAME", "SPHERE"
        );

        for (name, (sphere_identity, sphere_version)) in rows {
            let version =
                sphere_version.map_or_else(|| String::from("<none>"), |link| link.to_string());

            info!("{name:max_name_length$}  {sphere_identity:max_did_length$}  {version}");
        }
    }

    Ok(())
}
