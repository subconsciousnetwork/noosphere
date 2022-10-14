use crate::native::{commands::serve::tracing::initialize_tracing, workspace::Workspace};
use anyhow::{anyhow, Result};
use noosphere::{
    authority::{Authorization, SUPPORTED_KEYS},
    view::Sphere,
};
use noosphere_api::{
    client::Client,
    data::{FetchParameters, FetchResponse, PushBody, PushResponse},
};
use noosphere_storage::{
    db::SphereDb,
    interface::Store,
    memory::{MemoryStore},
};
use ucan::{
    crypto::{did::DidParser, KeyMaterial},
};

// TODO: If we fail before rendering, it will look like we have removed files
// from the workspace; we should probably roll back in the failure case.
pub async fn sync(workspace: &Workspace) -> Result<()> {
    initialize_tracing();
    workspace.expect_local_directories()?;

    let mut db = workspace.get_local_db().await?;
    let mut memory_store = MemoryStore::default();

    match workspace
        .get_local_content_changes(None, &db, &mut memory_store)
        .await?
    {
        Some((_, content_changes)) if !content_changes.is_empty() => {
            return Err(anyhow!(
                "You have unsaved local changes; save or revert them before syncing!"
            ));
        }
        _ => (),
    };

    let gateway_url = workspace.get_local_gateway_url().await?;

    let authorization = workspace.get_local_authorization().await?;
    let key = workspace.get_local_key().await?;

    let sphere_identity = workspace.get_local_identity().await?;

    let mut did_parser = DidParser::new(SUPPORTED_KEYS);

    println!("Handshaking with gateway at {}...", gateway_url);

    let client = Client::identify(
        &sphere_identity,
        &gateway_url,
        &key,
        &authorization,
        &mut did_parser,
        db.clone(),
    )
    .await?;

    sync_remote_changes(
        &sphere_identity,
        &client,
        &key,
        Some(&authorization),
        &mut db,
    )
    .await?;

    push_local_changes(&sphere_identity, &client, &mut db).await?;

    println!("Sync complete, rendering updated workspace...");

    workspace.render(&mut db).await?;

    println!("Done!");

    Ok(())
}

/// Attempts to push the latest local lineage to the gateway, causing the
/// gateway to update its own pointer to the tip of the local sphere's history
pub async fn push_local_changes<'a, S, K>(
    local_sphere_identity: &str,
    client: &Client<'a, K, SphereDb<S>>,
    db: &mut SphereDb<S>,
) -> Result<()>
where
    K: KeyMaterial,
    S: Store,
{
    let local_sphere_identity = local_sphere_identity.to_string();
    let counterpart_sphere_identity = client.session.sphere_identity.clone();

    // The base of the changes that must be pushed is the tip of our lineage as
    // recorded by the most recent history of the gateway's sphere. Everything
    // past that point in history represents new changes that the gateway does
    // not yet know about.
    let local_sphere_tip = db
        .get_version(&local_sphere_identity)
        .await?
        .ok_or_else(|| {
            anyhow!(
                "The history of local sphere {} is missing!",
                local_sphere_identity
            )
        })?;

    let counterpart_sphere_tip = db
        .get_version(&counterpart_sphere_identity)
        .await?
        .ok_or_else(|| {
            anyhow!(
                "No local history for counterpart sphere {}; did you forget to fetch?",
                counterpart_sphere_identity
            )
        })?;

    let local_sphere_base = Sphere::at(&counterpart_sphere_tip, db)
        .try_get_links()
        .await?
        .get(&local_sphere_identity)
        .await?
        .cloned();

    if local_sphere_base == Some(local_sphere_tip) {
        println!("Gateway is already up to date!");
        return Ok(());
    }

    println!("Collecting blocks from new local history...");

    let bundle = Sphere::at(&local_sphere_tip, db)
        .try_bundle_until_ancestor(local_sphere_base.as_ref())
        .await?;

    println!(
        "Pushing new local history to gateway {}...",
        client.session.gateway_identity
    );

    let result = client
        .push(&PushBody {
            sphere: local_sphere_identity,
            base: local_sphere_base,
            tip: local_sphere_tip,
            blocks: bundle,
        })
        .await?;

    let (counterpart_sphere_updated_tip, new_blocks) = match result {
        PushResponse::Accepted { new_tip, blocks } => (new_tip, blocks),
        PushResponse::NoChange => {
            return Err(anyhow!("Gateway already up to date!"));
        }
    };

    println!("Saving updated counterpart sphere history...");

    new_blocks.load_into(db).await?;

    Sphere::try_hydrate_range(
        Some(&counterpart_sphere_tip),
        &counterpart_sphere_updated_tip,
        db,
    )
    .await?;

    db.set_version(
        &counterpart_sphere_identity,
        &counterpart_sphere_updated_tip,
    )
    .await?;

    Ok(())
}

/// Fetches the latest changes from a gateway and updates the local lineage
/// using a conflict-free rebase strategy
pub async fn sync_remote_changes<'a, K, S>(
    local_sphere_identity: &str,
    client: &Client<'a, K, SphereDb<S>>,
    credential: &'a K,
    authorization: Option<&Authorization>,
    db: &mut SphereDb<S>,
) -> Result<()>
where
    K: KeyMaterial,
    S: Store,
{
    let local_sphere_identity = local_sphere_identity.to_string();
    let counterpart_sphere_identity = client.session.sphere_identity.clone();
    let counterpart_sphere_base = db.get_version(&counterpart_sphere_identity).await?;

    println!(
        "Fetching latest changes from gateway {}...",
        client.session.gateway_identity
    );

    let fetch_result = client
        .fetch(&FetchParameters {
            since: counterpart_sphere_base,
        })
        .await?;

    let (counterpart_sphere_tip, new_blocks) = match fetch_result {
        FetchResponse::NewChanges { tip, blocks } => (tip, blocks),
        FetchResponse::UpToDate => {
            println!("Local history is already up to date...");
            return Ok(());
        }
    };

    println!("Saving blocks to local database...");

    new_blocks.load_into(db).await?;

    println!("Hydrating received counterpart sphere revisions...");

    Sphere::try_hydrate_range(
        counterpart_sphere_base.as_ref(),
        &counterpart_sphere_tip,
        db,
    )
    .await?;

    db.set_version(&counterpart_sphere_identity, &counterpart_sphere_tip)
        .await?;

    let local_sphere_tip = db.get_version(&local_sphere_identity).await?;
    let local_sphere_old_base = match counterpart_sphere_base {
        Some(counterpart_sphere_base) => Sphere::at(&counterpart_sphere_base, db)
            .try_get_links()
            .await?
            .get(&local_sphere_identity)
            .await?
            .cloned(),
        None => None,
    };
    let local_sphere_new_base = Sphere::at(&counterpart_sphere_tip, db)
        .try_get_links()
        .await?
        .get(&local_sphere_identity)
        .await?
        .cloned();

    match (
        local_sphere_tip,
        local_sphere_old_base,
        local_sphere_new_base,
    ) {
        (Some(current_tip), Some(old_base), Some(new_base)) => {
            println!("Syncing received local sphere revisions...");
            let new_tip = Sphere::at(&current_tip, db)
                .try_sync(&old_base, &new_base, credential, authorization)
                .await?;

            db.set_version(&local_sphere_identity, &new_tip).await?;
        }
        (None, old_base, Some(new_base)) => {
            println!("Hydrating received local sphere revisions...");
            Sphere::try_hydrate_range(old_base.as_ref(), &new_base, db).await?;

            db.set_version(&local_sphere_identity, &new_base).await?;
        }
        _ => {
            println!("Nothing to sync!");
            return Ok(());
        }
    };

    Ok(())
}
