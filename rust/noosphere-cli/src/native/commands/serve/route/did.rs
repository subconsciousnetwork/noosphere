use std::sync::Arc;

use axum::{http::StatusCode, Extension};
use ucan::crypto::KeyMaterial;

pub async fn did_route<K: KeyMaterial>(
    Extension(gateway_key): Extension<Arc<K>>,
) -> Result<String, StatusCode> {
    gateway_key.get_did().await.map_err(|error| {
        error!("{:?}", error);
        StatusCode::INTERNAL_SERVER_ERROR
    })
}
