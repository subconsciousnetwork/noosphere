use noosphere_api::data::PushError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum NoosphereError {
    #[error("{0}")]
    Other(String),

    #[error("Network access required but network is currently offline")]
    NetworkOffline,

    #[error("No credentials configured")]
    NoCredentials,

    #[error("Missing configuration: {0}")]
    MissingConfiguration(&'static str),
}

impl From<std::io::Error> for NoosphereError {
    fn from(value: std::io::Error) -> Self {
        NoosphereError::Other(value.to_string())
    }
}

impl From<String> for NoosphereError {
    fn from(error: String) -> Self {
        NoosphereError::Other(error)
    }
}

impl From<anyhow::Error> for NoosphereError {
    fn from(error: anyhow::Error) -> Self {
        NoosphereError::Other(error.to_string())
    }
}

impl From<NoosphereError> for PushError {
    fn from(error: NoosphereError) -> Self {
        error.into()
    }
}
