use anyhow::{anyhow, Result};
use async_std::sync::Mutex;
use hyper::StatusCode;
use noosphere::view::{Sphere, Timeline};
use noosphere_api::data::PushResponse;
use noosphere_storage::interface::KeyValueStore;
use std::sync::Arc;

use axum::debug_handler;
use axum::{extract::ContentLengthLimit, Extension, Json};
use noosphere_api::{
    authority::GatewayAction,
    data::{MissingRevisionsResponse, OutOfDateResponse, PushBody},
};
use noosphere_storage::{interface::DagCborStore, native::NativeStore};

use crate::gateway::environment::{Blocks, SphereTracker};
use crate::gateway::extractors::DagCbor;
use crate::gateway::{authority::GatewayAuthority, environment::GatewayState, GatewayError};

async fn incorporate_lineage(
    state: Arc<Mutex<GatewayState<NativeStore>>>,
    store: Arc<Mutex<Blocks<NativeStore>>>,
    push_body: &PushBody,
) -> Result<()> {
    let mut store = store.lock().await.clone();

    for (expected_cid, block) in push_body.blocks.map() {
        let actual_cid = store.write_cbor(block).await?;
        if expected_cid != &actual_cid {
            return Err(anyhow!("Invalid block"));
        }
    }

    let PushBody { base, tip, .. } = push_body;

    let timeline = Timeline::new(&store);
    let timeslice = timeline.slice(tip, base.as_ref());
    let steps = timeslice.try_to_chronological().await?;

    for (cid, _) in steps {
        Sphere::at(&cid, &store).try_hydrate().await?;
    }

    let mut state = state.lock().await;
    let mut tracker = state.get_or_initialize_tracker(&push_body.sphere).await?;

    tracker.latest = Some(push_body.tip.clone());

    state.set(&push_body.sphere, &tracker).await?;

    Ok(())
}

#[debug_handler]
pub async fn push_handler(
    authority: GatewayAuthority,
    ContentLengthLimit(DagCbor(push_body)): ContentLengthLimit<DagCbor<PushBody>, { 1024 * 5000 }>,
    Extension(state): Extension<Arc<Mutex<GatewayState<NativeStore>>>>,
    Extension(store): Extension<Arc<Mutex<Blocks<NativeStore>>>>,
) -> Result<(StatusCode, Json<PushResponse>), GatewayError> {
    authority.try_authorize(GatewayAction::Push).await?;

    let SphereTracker { latest: tip, .. } = state
        .lock()
        .await
        .get_or_initialize_tracker(&push_body.sphere)
        .await?;

    match (tip, push_body.base) {
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
                return Ok((
                    StatusCode::CONFLICT,
                    Json(PushResponse::OutOfDate(OutOfDateResponse {
                        sphere: push_body.sphere,
                        presumed_base: theirs,
                        actual_tip: mine,
                    })),
                ));
            }

            incorporate_lineage(state, store, &push_body).await?;

            Ok((StatusCode::OK, Json(PushResponse::Ok)))
        }
        (None, Some(theirs)) => Ok((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(PushResponse::MissingRevisions(MissingRevisionsResponse {
                sphere: push_body.sphere,
                presumed_base: theirs,
                actual_tip: None,
            })),
        )),
        (None, None) => {
            incorporate_lineage(state, store, &push_body).await?;
            Ok((StatusCode::OK, Json(PushResponse::Ok)))
        }
    }
}
