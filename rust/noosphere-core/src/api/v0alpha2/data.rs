use crate::{
    api::StatusCode,
    data::{Did, Jwt, Link, MemoIpld},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The body payload expected by the "push" API route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushBody {
    /// The DID of the local sphere whose revisions are being pushed
    pub sphere: Did,
    /// The base revision represented by the payload being pushed; if the
    /// entire history is being pushed, then this should be None
    pub local_base: Option<Link<MemoIpld>>,
    /// The tip of the history represented by the payload being pushed
    pub local_tip: Link<MemoIpld>,
    /// The last received tip of the counterpart sphere
    pub counterpart_tip: Option<Link<MemoIpld>>,
    /// An optional name record to publish to the Noosphere Name System
    pub name_record: Option<Jwt>,
}

/// The possible responses from the "push" API route
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PushResponse {
    /// The new history was accepted
    Accepted {
        /// This is the new tip of the "counterpart" sphere after accepting
        /// the latest history from the local sphere. This is guaranteed to be
        /// at least one revision ahead of the latest revision being tracked
        /// by the client (because it points to the newly received tip of the
        /// local sphere's history)
        new_tip: Link<MemoIpld>,
    },
    /// The history was already known by the API host, so no changes were made
    NoChange,
}

/// Error types for typical "push" API failure conditions
#[allow(missing_docs)]
#[derive(Serialize, Deserialize, Error, Debug)]
pub enum PushError {
    #[error("Stream of blocks up to the gateway was interrupted")]
    BrokenUpstream,
    #[error("Stream of blocks down from the gateway was interrupted")]
    BrokenDownstream,
    #[error("First block in upstream was missing or unexpected type")]
    UnexpectedBody,
    #[error("Pushed history conflicts with canonical history")]
    Conflict,
    #[error("Missing some implied history")]
    MissingHistory,
    #[error("Replica is up to date")]
    UpToDate,
    #[error("Internal error: {0:?}")]
    Internal(Option<String>),
}

impl From<&PushError> for StatusCode {
    fn from(value: &PushError) -> Self {
        match value {
            PushError::BrokenUpstream => StatusCode::BAD_GATEWAY,
            PushError::BrokenDownstream => StatusCode::BAD_GATEWAY,
            PushError::UnexpectedBody => StatusCode::UNPROCESSABLE_ENTITY,
            PushError::Conflict => StatusCode::CONFLICT,
            PushError::MissingHistory => StatusCode::FAILED_DEPENDENCY,
            PushError::UpToDate => StatusCode::NOT_MODIFIED,
            PushError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<anyhow::Error> for PushError {
    fn from(value: anyhow::Error) -> Self {
        PushError::Internal(Some(format!("{value}")))
    }
}
