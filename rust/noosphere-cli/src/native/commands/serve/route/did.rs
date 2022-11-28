use axum::{http::StatusCode, Extension};
use noosphere_core::data::Did;
use ucan::crypto::KeyMaterial;

pub async fn did_route<K: KeyMaterial>(
    Extension(gateway_identity): Extension<Did>,
) -> Result<String, StatusCode> {
    Ok(gateway_identity.into())
}
