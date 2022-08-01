use axum::{response::IntoResponse, Extension, Json};
use noosphere_api::data::IdentifyResponse;
use std::sync::Arc;

use crate::gateway::{environment::GatewayConfig, GatewayError};

pub async fn identify_handler(
    Extension(config): Extension<Arc<GatewayConfig>>,
) -> Result<impl IntoResponse, GatewayError> {
    let identity_did = config.expect_identity().await?;

    Ok(Json(IdentifyResponse {
        identity: identity_did,
    }))
}
