use thiserror::Error;

/// Car utility error
#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed to parse CAR file: {0}")]
    Parsing(String),
    #[error("Invalid CAR file: {0}")]
    InvalidFile(String),
    #[error("Io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Cbor encoding error: {0}")]
    Cbor(#[from] libipld::error::Error),
    #[error("ld read too large {0}")]
    LdReadTooLarge(usize),
}

impl From<cid::Error> for Error {
    fn from(err: cid::Error) -> Error {
        Error::Parsing(err.to_string())
    }
}

impl From<cid::multihash::Error> for Error {
    fn from(err: cid::multihash::Error) -> Error {
        Error::Parsing(err.to_string())
    }
}
