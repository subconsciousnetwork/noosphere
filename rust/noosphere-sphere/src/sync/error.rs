use noosphere_api::data::PushError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SyncError {
    #[error("There was a conflict during sync")]
    Conflict,
    #[error("{0}")]
    Other(anyhow::Error),
}

impl From<anyhow::Error> for SyncError {
    fn from(value: anyhow::Error) -> Self {
        SyncError::Other(value)
    }
}

impl From<PushError> for SyncError {
    fn from(value: PushError) -> Self {
        match value {
            PushError::Conflict => SyncError::Conflict,
            any => SyncError::Other(any.into()),
        }
    }
}
