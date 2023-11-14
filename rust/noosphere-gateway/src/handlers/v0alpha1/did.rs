use crate::extractors::GatewayScope;
use axum::http::StatusCode;
use noosphere_core::context::HasMutableSphereContext;
use noosphere_storage::Storage;

pub async fn did_route<C, S>(scope: GatewayScope<C, S>) -> Result<String, StatusCode>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    Ok(scope.gateway_identity.into())
}
