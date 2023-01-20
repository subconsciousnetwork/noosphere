use std::sync::Arc;

use anyhow::Result;

use axum::{extract::ContentLengthLimit, http::StatusCode, Extension};

use cid::Cid;
use noosphere::sphere::SphereContext;
use noosphere_api::data::{PushBody, PushResponse};
use noosphere_core::{
    authority::{Authorization, SphereAction, SphereReference},
    data::Bundle,
    view::{Sphere, SphereMutation, Timeline},
};
use noosphere_storage::{NativeStorage, SphereDb};
use tokio::sync::{mpsc::UnboundedSender, Mutex};
use ucan::capability::{Capability, Resource, With};
use ucan::crypto::KeyMaterial;

use crate::native::commands::serve::{
    authority::GatewayAuthority, extractor::Cbor, gateway::GatewayScope,
    ipfs::SyndicationJob,
};

// #[debug_handler]
pub async fn push_route<K>(
    authority: GatewayAuthority<K>,
    ContentLengthLimit(Cbor(push_body)): ContentLengthLimit<Cbor<PushBody>, { 1024 * 5000 }>,
    Extension(sphere_context_mutex): Extension<Arc<Mutex<SphereContext<K, NativeStorage>>>>,
    Extension(scope): Extension<GatewayScope>,
    Extension(syndication_tx): Extension<UnboundedSender<SyndicationJob<K, NativeStorage>>>,
) -> Result<Cbor<PushResponse>, StatusCode>
where
    K: KeyMaterial + Clone + 'static,
{
    debug!("Invoking push route...");

    let sphere_identity = &push_body.sphere;

    if sphere_identity != &scope.counterpart {
        return Err(StatusCode::FORBIDDEN);
    }

    authority.try_authorize(&Capability {
        with: With::Resource {
            kind: Resource::Scoped(SphereReference {
                did: scope.counterpart.to_string(),
            }),
        },
        can: SphereAction::Push,
    })?;

    let sphere_context = sphere_context_mutex.lock().await;
    let mut db = sphere_context.db().clone();
    let gateway_key = &sphere_context.author().key;
    let gateway_authorization =
        sphere_context
            .author()
            .require_authorization()
            .map_err(|error| {
                error!("{:?}", error);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

    debug!("Preparing to merge sphere lineage...");
    let local_sphere_base_cid = db.get_version(sphere_identity).await.map_err(|error| {
        error!("{:?}", error);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let request_sphere_base_cid = push_body.base;

    match (local_sphere_base_cid, request_sphere_base_cid) {
        (Some(mine), theirs) => {
            // TODO(#26): Probably should do some diligence here to check if
            // their base is even in our lineage. Note that this condition
            // will be hit if theirs is ahead of mine, which actually
            // should be a "missing revisions" condition.
            let conflict = match theirs {
                Some(cid) if cid != mine => true,
                None => true,
                _ => false,
            };

            if conflict {
                warn!("Conflict!");
                return Err(StatusCode::CONFLICT);
            }

            if push_body.tip == mine {
                warn!("No new changes in push body!");
                return Ok(Cbor(PushResponse::NoChange));
            }
        }
        (None, Some(_)) => {
            error!("Missing local lineage!");
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
        _ => (),
    };

    debug!("Merging...");

    incorporate_lineage(&scope, &mut db, &push_body)
        .await
        .map_err(|error| {
            error!("{:?}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    debug!("Updating the gateway's sphere...");

    let (new_gateway_tip, new_blocks) = update_gateway_sphere(
        &push_body.tip,
        &scope,
        gateway_key,
        gateway_authorization,
        &mut db,
    )
    .await
    .map_err(|error| {
        error!("{:?}", error);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // TODO(#156): This should not be happening on every push, but rather on
    // an explicit publish action. Move this to the publish handler when we
    // have added it to the gateway.
    if let Err(error) = syndication_tx.send(SyndicationJob {
        revision: new_gateway_tip,
        context: sphere_context_mutex.clone(),
    }) {
        warn!("Failed to queue IPFS syndication job: {}", error);
    };

    Ok(Cbor(PushResponse::Accepted {
        new_tip: new_gateway_tip,
        blocks: new_blocks,
    }))
}

async fn update_gateway_sphere<K>(
    counterpart_sphere_cid: &Cid,
    scope: &GatewayScope,
    key: &K,
    authority: &Authorization,
    db: &mut SphereDb<NativeStorage>,
) -> Result<(Cid, Bundle)>
where
    K: KeyMaterial + Send,
{
    let my_sphere_cid = db.require_version(&scope.identity).await?;

    let my_sphere = Sphere::at(&my_sphere_cid, db);
    let my_did = key.get_did().await?;

    let mut mutation = SphereMutation::new(&my_did);
    mutation
        .links_mut()
        .set(&scope.counterpart, counterpart_sphere_cid);

    let mut revision = my_sphere.try_apply_mutation(&mutation).await?;

    let my_updated_sphere_cid = revision.try_sign(key, Some(authority)).await?;

    db.set_version(&scope.identity, &my_updated_sphere_cid)
        .await?;

    let blocks = Sphere::at(&my_updated_sphere_cid, db)
        .try_bundle_until_ancestor(Some(&my_sphere_cid))
        .await?;

    Ok((my_updated_sphere_cid, blocks))
}

async fn incorporate_lineage(
    scope: &GatewayScope,
    db: &mut SphereDb<NativeStorage>,
    push_body: &PushBody,
) -> Result<()> {
    push_body.blocks.load_into(db).await?;

    let PushBody { base, tip, .. } = push_body;

    let timeline = Timeline::new(db);
    let timeslice = timeline.slice(tip, base.as_ref());
    let steps = timeslice.try_to_chronological().await?;

    for (cid, _) in steps {
        debug!("Hydrating {}", cid);
        Sphere::at(&cid, db).try_hydrate().await?;
    }

    db.set_version(&scope.counterpart, &push_body.tip).await?;

    Ok(())
}
