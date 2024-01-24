use crate::extractors::{GatewayAuthority, GatewayScope};
use crate::GatewayManager;
use axum::{http::StatusCode, response::IntoResponse, Json};
use noosphere_core::api::v0alpha1::IdentifyResponse;
use noosphere_core::authority::SphereAbility;
use noosphere_core::context::HasMutableSphereContext;
use noosphere_storage::Storage;

pub async fn identify_route<M, C, S>(
    gateway_scope: GatewayScope<C, S>,
    authority: GatewayAuthority<M, C, S>,
) -> Result<impl IntoResponse, StatusCode>
where
    M: GatewayManager<C, S> + 'static,
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    debug!("Invoking identify route...");
    let gateway_sphere = authority
        .try_authorize(&gateway_scope, SphereAbility::Fetch)
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
