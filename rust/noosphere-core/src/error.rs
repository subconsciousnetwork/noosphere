use crate::authority::Authorization;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum NoosphereError {
    #[error("{0}")]
    Other(anyhow::Error),

    #[error("Network access required but network is currently offline")]
    NetworkOffline,

    #[error("No credentials configured")]
    NoCredentials,

    #[error("Missing configuration: {0}")]
    MissingConfiguration(&'static str),

    #[error("The provided authorization {0} is invalid: {1}")]
    InvalidAuthorization(Authorization, String),
}

impl From<anyhow::Error> for NoosphereError {
    fn from(error: anyhow::Error) -> Self {
        NoosphereError::Other(error)
    }
}
