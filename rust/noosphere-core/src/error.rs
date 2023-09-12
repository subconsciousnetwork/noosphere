//! Noosphere errors

use crate::authority::Authorization;
use thiserror::Error;

/// High-level error types relevant to the Noosphere protocol
#[derive(Error, Debug)]
pub enum NoosphereError {
    /// Any error not covered by the other errors
    #[error("{0}")]
    Other(anyhow::Error),

    #[allow(missing_docs)]
    #[error("Network access required but network is currently offline")]
    NetworkOffline,

    #[allow(missing_docs)]
    #[error("No credentials configured")]
    NoCredentials,

    #[allow(missing_docs)]
    #[error("Missing configuration: {0}")]
    MissingConfiguration(&'static str),

    #[allow(missing_docs)]
    #[error("The provided authorization {0} is invalid: {1}")]
    InvalidAuthorization(Authorization, String),
}

impl From<anyhow::Error> for NoosphereError {
    fn from(error: anyhow::Error) -> Self {
        NoosphereError::Other(error)
    }
}
