use std::sync::Arc;

use axum::{http::StatusCode, response::IntoResponse, Extension, Json};
use noosphere::authority::{SphereAction, SphereReference};
use noosphere_api::data::IdentifyResponse;
use ucan::{
    capability::{Capability, Resource, With},
    crypto::KeyMaterial,
    Ucan,
};

use crate::native::commands::serve::{authority::GatewayAuthority, gateway::GatewayScope};

pub async fn identify_route<K: KeyMaterial>(
    authority: GatewayAuthority,
    Extension(gateway_key): Extension<Arc<K>>,
    Extension(gateway_authority): Extension<Ucan>,
    Extension(scope): Extension<GatewayScope>,
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

    Ok(Json(
        IdentifyResponse::sign(&scope.identity, &gateway_key, &gateway_authority)
            .await
            .map_err(|error| {
                error!("{:?}", error);
                StatusCode::INTERNAL_SERVER_ERROR
            })?,
    ))
}
