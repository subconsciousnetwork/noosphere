use anyhow;
use libp2p::{kad, kad::record::store::Error as KadStorageError, TransportError};
use noosphere_common::channel::ChannelError;
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum DhtError {
    Error(String),
    IO(io::ErrorKind),
    ValidationError(Vec<u8>),
    LibP2PTransportError(Option<libp2p::Multiaddr>),
    LibP2PStorageError(KadStorageError),
    LibP2PGetRecordError(kad::GetRecordError),
    LibP2PBootstrapError(kad::BootstrapError),
    LibP2PPutRecordError(kad::PutRecordError),
    LibP2PAddProviderError(kad::AddProviderError),
    LibP2PGetProvidersError(kad::GetProvidersError),
    NotConnected,
    NoKnownPeers,
}

impl std::error::Error for DhtError {}
impl fmt::Display for DhtError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DhtError::NotConnected => write!(fmt, "DHT not running"),
            DhtError::NoKnownPeers => write!(fmt, "no known peers"),
            DhtError::LibP2PTransportError(e) => write!(fmt, "{e:#?}"),
            DhtError::LibP2PStorageError(e) => write!(fmt, "{e:#?}"),
            DhtError::LibP2PGetRecordError(e) => write!(fmt, "{e:#?}"),
            DhtError::LibP2PPutRecordError(e) => write!(fmt, "{e:#?}"),
            DhtError::LibP2PBootstrapError(e) => write!(fmt, "{e:#?}"),
            DhtError::LibP2PAddProviderError(e) => write!(fmt, "{e:#?}"),
            DhtError::LibP2PGetProvidersError(e) => write!(fmt, "{e:#?}"),
            DhtError::IO(k) => write!(fmt, "{k:#?}"),
            DhtError::Error(m) => write!(fmt, "{m:#?}"),
            DhtError::ValidationError(_) => write!(fmt, "validation error"),
        }
    }
}

impl From<ChannelError> for DhtError {
    fn from(e: ChannelError) -> Self {
        match e {
            ChannelError::RecvError => DhtError::Error("RecvError".into()),
            ChannelError::SendError => DhtError::Error("SendError".into()),
        }
    }
}

impl From<anyhow::Error> for DhtError {
    fn from(e: anyhow::Error) -> Self {
        DhtError::Error(e.to_string())
    }
}

impl From<io::Error> for DhtError {
    fn from(e: io::Error) -> Self {
        DhtError::IO(e.kind())
    }
}

impl<TErr> From<TransportError<TErr>> for DhtError {
    fn from(e: TransportError<TErr>) -> Self {
        match e {
            TransportError::MultiaddrNotSupported(addr) => {
                DhtError::LibP2PTransportError(Some(addr))
            }
            TransportError::Other(_) => DhtError::LibP2PTransportError(None),
        }
    }
}

impl From<KadStorageError> for DhtError {
    fn from(e: KadStorageError) -> Self {
        DhtError::LibP2PStorageError(e)
    }
}

impl From<kad::GetRecordError> for DhtError {
    fn from(e: kad::GetRecordError) -> Self {
        DhtError::LibP2PGetRecordError(e)
    }
}

impl From<kad::PutRecordError> for DhtError {
    fn from(e: kad::PutRecordError) -> Self {
        DhtError::LibP2PPutRecordError(e)
    }
}

impl From<kad::BootstrapError> for DhtError {
    fn from(e: kad::BootstrapError) -> Self {
        DhtError::LibP2PBootstrapError(e)
    }
}

impl From<kad::AddProviderError> for DhtError {
    fn from(e: kad::AddProviderError) -> Self {
        DhtError::LibP2PAddProviderError(e)
    }
}

impl From<kad::GetProvidersError> for DhtError {
    fn from(e: kad::GetProvidersError) -> Self {
        DhtError::LibP2PGetProvidersError(e)
    }
}
