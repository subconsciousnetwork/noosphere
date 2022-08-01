use anyhow::{anyhow, Result};
use async_std::sync::Mutex;
use hyper::StatusCode;
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

use crate::gateway::environment::{BlockStore, SphereTracker};
use crate::gateway::extractors::DagCbor;
use crate::gateway::{authority::GatewayAuthority, environment::GatewayState, GatewayError};

async fn incorporate_lineage(
    state: Arc<Mutex<GatewayState<NativeStore>>>,
    store: Arc<Mutex<BlockStore<NativeStore>>>,
    push_body: &PushBody,
) -> Result<()> {
    debug!("INCORPORATING LINEAGE {:?}", push_body);
    for (expected_cid, block) in push_body.blocks.map() {
        debug!("WRITING BLOCK {}", expected_cid);
        let actual_cid = store.lock().await.write_cbor(block).await?;
        if expected_cid != &actual_cid {
            return Err(anyhow!("Invalid block"));
        }
    }

    // TODO: Hydrate

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
    Extension(store): Extension<Arc<Mutex<BlockStore<NativeStore>>>>,
) -> Result<(StatusCode, Json<PushResponse>), GatewayError> {
    authority.try_authorize(GatewayAction::Push).await?;

    let SphereTracker { latest: tip, .. } = state
        .lock()
        .await
        .get_or_initialize_tracker(&push_body.sphere)
        .await?;

    match (tip, push_body.base) {
        (Some(mine), theirs) => {
            // TODO: Probably should do some diligence here to check if
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
