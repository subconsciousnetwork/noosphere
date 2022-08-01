use std::{error::Error, fmt::Display};

use axum::{
    response::{IntoResponse, Response},
    Json,
};
use hyper::StatusCode;
use serde_json::json;

pub enum GatewayError {
    Internal(anyhow::Error),
    Authz(AuthzError),
}

impl From<anyhow::Error> for GatewayError {
    fn from(error: anyhow::Error) -> Self {
        GatewayError::Internal(error)
    }
}

impl IntoResponse for GatewayError {
    fn into_response(self) -> axum::response::Response {
        match self {
            GatewayError::Internal(error) => {
                error!("Internal server error: {:?}", error);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
            }
            GatewayError::Authz(authz) => authz.into_response(),
        }
    }
}

#[derive(Debug)]
pub enum AuthzError {
    /// The server is misconfigured
    Internal(String),
    /// The bearer header is malformed or missing
    MalformedToken,
    /// The UCAN is not valid (expired, or invalid signature, or the witness
    /// chain is invalid)
    InvalidCredentials,
    /// The requested capability is not enabled with the supplied credentials
    WrongCredentials,
}

impl IntoResponse for AuthzError {
    fn into_response(self) -> Response {
        let status = match &self {
            AuthzError::Internal(reason) => {
                error!("Authz internal error: {}", reason);
                StatusCode::INTERNAL_SERVER_ERROR
            }
            AuthzError::MalformedToken => StatusCode::BAD_REQUEST,
            AuthzError::InvalidCredentials => StatusCode::UNAUTHORIZED,
            AuthzError::WrongCredentials => StatusCode::UNAUTHORIZED,
        };
        let body = Json(json!({
            "error": self.to_string(),
        }));
        (status, body).into_response()
    }
}

impl Display for AuthzError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                AuthzError::Internal(_) => "Unexpected server error",
                AuthzError::MalformedToken => "Malformed or missing bearer token",
                AuthzError::InvalidCredentials => "Invalid bearer token",
                AuthzError::WrongCredentials => "Credentials are not sufficient",
            }
        )
    }
}

impl Error for AuthzError {}
