use crate::api::v0alpha2::PushError;
use thiserror::Error;

/// Different classes of error that may occur during synchronization with a
/// gateway
#[derive(Error, Debug)]
pub enum SyncError {
    /// The error was due to not having write access to the sphere
    #[error("Insufficient permission to sync")]
    InsufficientPermission,
    /// The error was a conflict; this is possibly recoverable
    #[error("There was a conflict during sync")]
    Conflict,
    /// The error was some other, non-specific error
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
