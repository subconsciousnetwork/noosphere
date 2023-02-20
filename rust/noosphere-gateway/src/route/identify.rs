use std::sync::Arc;

use axum::{http::StatusCode, response::IntoResponse, Extension, Json};
use noosphere::sphere::SphereContext;
use noosphere_api::data::IdentifyResponse;
use noosphere_core::authority::{SphereAction, SphereReference};
use noosphere_storage::NativeStorage;
use tokio::sync::Mutex;
use ucan::{
    capability::{Capability, Resource, With},
    crypto::KeyMaterial,
};

use crate::{authority::GatewayAuthority, GatewayScope};

pub async fn identify_route<K: KeyMaterial + Clone>(
    authority: GatewayAuthority<K>,
    Extension(scope): Extension<GatewayScope>,
    Extension(sphere_context): Extension<Arc<Mutex<SphereContext<K, NativeStorage>>>>,
) -> Result<impl IntoResponse, StatusCode> {
    debug!("Invoking identify route...");

    authority.try_authorize(&Capability {
        with: With::Resource {
            kind: Resource::Scoped(SphereReference {
                did: scope.counterpart.to_string(),
            }),
        },
        can: SphereAction::Fetch,
    })?;

    let sphere_context = sphere_context.lock().await;
    let db = sphere_context.db();
    let gateway_key = &sphere_context.author().key;
    let gateway_authorization =
        sphere_context
            .author()
            .require_authorization()
            .map_err(|error| {
                error!("{:?}", error);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

    let ucan = gateway_authorization
        .resolve_ucan(db)
        .await
        .map_err(|error| {
            error!("{:?}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(
        IdentifyResponse::sign(&scope.identity, gateway_key, &ucan)
            .await
            .map_err(|error| {
                error!("{:?}", error);
                StatusCode::INTERNAL_SERVER_ERROR
            })?,
    ))
}
