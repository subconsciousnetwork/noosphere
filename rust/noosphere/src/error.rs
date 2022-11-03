use thiserror::Error;

#[derive(Error, Debug)]
pub enum NoosphereError {
    #[error("Network access required but network is currently offline")]
    NetworkOffline,

    #[error("No credentials configured")]
    NoCredentials,

    #[error("Missing configuration: {0}")]
    MissingConfiguration(&'static str),

    #[error("{0}")]
    Other(anyhow::Error),
}

impl From<anyhow::Error> for NoosphereError {
    fn from(error: anyhow::Error) -> Self {
        NoosphereError::Other(error)
    }
}
