//! Axum extractors, used to create arguments in routes from a request.

mod authority;
mod cbor;
mod scope;

pub use authority::*;
pub use cbor::*;
pub use scope::*;

pub(crate) fn map_bad_request<E: std::fmt::Debug>(error: E) -> axum::http::StatusCode {
    tracing::error!("{:?}", error);
    axum::http::StatusCode::BAD_REQUEST
}
