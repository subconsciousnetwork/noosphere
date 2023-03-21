use axum::{http::StatusCode, Extension};
use noosphere_core::data::Did;

pub async fn did_route(Extension(gateway_identity): Extension<Did>) -> Result<String, StatusCode> {
    Ok(gateway_identity.into())
}
