use std::sync::Arc;

use axum::{http::StatusCode, response::IntoResponse, Extension, Json};
use noosphere::authority::{Authorization, SphereAction, SphereReference};
use noosphere_api::data::IdentifyResponse;
use noosphere_storage::{db::SphereDb, native::NativeStore};
use ucan::{
    capability::{Capability, Resource, With},
    crypto::KeyMaterial,
};

use crate::native::commands::serve::{
    authority::GatewayAuthority,
    gateway::{GatewayScope},
};

pub async fn identify_route<K: KeyMaterial>(
    authority: GatewayAuthority,
    Extension(gateway_key): Extension<Arc<K>>,
    Extension(gateway_authorization): Extension<Authorization>,
    Extension(scope): Extension<GatewayScope>,
    Extension(db): Extension<SphereDb<NativeStore>>,
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

    let ucan = gateway_authorization
        .resolve_ucan(&db)
        .await
        .map_err(|error| {
            error!("{:?}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(
        IdentifyResponse::sign(&scope.identity, &gateway_key, &ucan)
            .await
            .map_err(|error| {
                error!("{:?}", error);
                StatusCode::INTERNAL_SERVER_ERROR
            })?,
    ))
}
