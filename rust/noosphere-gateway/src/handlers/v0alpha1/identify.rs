use crate::{authority::GatewayAuthority, GatewayScope};
use axum::{http::StatusCode, response::IntoResponse, Extension, Json};
use noosphere_core::api::v0alpha1::IdentifyResponse;
use noosphere_core::authority::{generate_capability, SphereAbility};
use noosphere_core::context::HasSphereContext;
use noosphere_storage::Storage;

pub async fn identify_route<C, S>(
    Extension(scope): Extension<GatewayScope>,
    Extension(sphere_context): Extension<C>,
    authority: GatewayAuthority,
) -> Result<impl IntoResponse, StatusCode>
where
    C: HasSphereContext<S>,
    S: Storage + 'static,
{
    debug!("Invoking identify route...");

    authority.try_authorize(&generate_capability(
        &scope.counterpart,
        SphereAbility::Fetch,
    ))?;

    let sphere_context = sphere_context
        .sphere_context()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let db = sphere_context.db();
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
        IdentifyResponse::sign(&scope.identity, gateway_key, &ucan)
            .await
            .map_err(|error| {
                error!("{:?}", error);
                StatusCode::INTERNAL_SERVER_ERROR
            })?,
    ))
}
