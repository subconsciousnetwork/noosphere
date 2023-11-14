use crate::extractors::{GatewayAuthority, GatewayScope, SphereExtractor};
use axum::{http::StatusCode, response::IntoResponse, Json};
use noosphere_core::api::v0alpha1::IdentifyResponse;
use noosphere_core::authority::{generate_capability, SphereAbility};
use noosphere_core::context::HasMutableSphereContext;
use noosphere_storage::Storage;

pub async fn identify_route<C, S>(
    authority: GatewayAuthority,
    sphere_extractor: SphereExtractor<C, S>,
    gateway_scope: GatewayScope<C, S>,
) -> Result<impl IntoResponse, StatusCode>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    debug!("Invoking identify route...");
    let mut gateway_sphere = sphere_extractor.into_inner();
    let counterpart = &gateway_scope.counterpart;
    authority
        .try_authorize(
            &mut gateway_sphere,
            counterpart,
            &generate_capability(counterpart.as_str(), SphereAbility::Fetch),
        )
        .await?;

    let sphere_context = gateway_sphere
        .sphere_context()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let db = sphere_context.db();
    let identity = sphere_context.identity();
    let gateway_key = &sphere_context.author().key;
    let gateway_authorization =
        sphere_context
            .author()
            .require_authorization()
            .map_err(|error| {
                error!("Could not find authorization: {:?}", error);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

    let ucan = gateway_authorization.as_ucan(db).await.map_err(|error| {
        error!("{:?}", error);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(
        IdentifyResponse::sign(identity, gateway_key, &ucan)
            .await
            .map_err(|error| {
                error!("{:?}", error);
                StatusCode::INTERNAL_SERVER_ERROR
            })?,
    ))
}
