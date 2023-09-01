use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use noosphere_core::api::v0alpha2::PushError;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct GatewayError {
    pub error: String,
}

pub struct GatewayErrorResponse(StatusCode, GatewayError);

impl IntoResponse for GatewayErrorResponse {
    fn into_response(self) -> axum::response::Response {
        (self.0, Json(self.1)).into_response()
    }
}

impl From<anyhow::Error> for GatewayErrorResponse {
    fn from(value: anyhow::Error) -> Self {
        GatewayErrorResponse(
            StatusCode::INTERNAL_SERVER_ERROR,
            GatewayError {
                error: value.to_string(),
            },
        )
    }
}

impl From<PushError> for GatewayErrorResponse {
    fn from(value: PushError) -> Self {
        GatewayErrorResponse(
            StatusCode::from(&value),
            GatewayError {
                error: value.to_string(),
            },
        )
    }
}

impl From<StatusCode> for GatewayErrorResponse {
    fn from(value: StatusCode) -> Self {
        GatewayErrorResponse(
            value,
            GatewayError {
                error: value.to_string(),
            },
        )
    }
}

impl From<axum::Error> for GatewayErrorResponse {
    fn from(value: axum::Error) -> Self {
        GatewayErrorResponse(
            StatusCode::INTERNAL_SERVER_ERROR,
            GatewayError {
                error: value.to_string(),
            },
        )
    }
}
