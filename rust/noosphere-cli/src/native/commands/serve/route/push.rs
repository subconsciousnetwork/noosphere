use anyhow::Result;
use axum::Json;
use axum::{extract::ContentLengthLimit, http::StatusCode, Extension};

use noosphere::authority::{SphereAction, SphereReference};
use noosphere::view::{Sphere, Timeline};
use noosphere_api::data::{PushBody, PushResponse};
use noosphere_storage::{db::SphereDb, native::NativeStore};
use ucan::capability::{Capability, Resource, With};

use crate::native::commands::serve::gateway::GatewayScope;
use crate::native::commands::serve::{authority::GatewayAuthority, extractor::Cbor};

// #[debug_handler]
pub async fn push_route(
    authority: GatewayAuthority,
    ContentLengthLimit(Cbor(push_body)): ContentLengthLimit<Cbor<PushBody>, { 1024 * 5000 }>,
    Extension(mut db): Extension<SphereDb<NativeStore>>,
    Extension(scope): Extension<GatewayScope>,
) -> Result<Json<PushResponse>, StatusCode> {
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

    debug!("Preparing to merge sphere lineage...");
    let local_sphere_base_cid = db.get_version(&sphere_identity).await.map_err(|error| {
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

    Ok(Json(PushResponse::Ok))
}

async fn incorporate_lineage(
    scope: &GatewayScope,
    db: &mut SphereDb<NativeStore>,
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
