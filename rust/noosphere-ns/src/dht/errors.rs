use crate::dht::channel::ChannelError;
use anyhow;
use libp2p::{kad, kad::record::store::Error as KadStorageError, TransportError};
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum DHTError {
    Error(String),
    IO(io::ErrorKind),
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

impl std::error::Error for DHTError {}
impl fmt::Display for DHTError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DHTError::NotConnected => write!(fmt, "DHT not running"),
            DHTError::NoKnownPeers => write!(fmt, "no known peers"),
            DHTError::LibP2PTransportError(e) => write!(fmt, "{:#?}", e),
            DHTError::LibP2PStorageError(e) => write!(fmt, "{:#?}", e),
            DHTError::LibP2PGetRecordError(e) => write!(fmt, "{:#?}", e),
            DHTError::LibP2PPutRecordError(e) => write!(fmt, "{:#?}", e),
            DHTError::LibP2PBootstrapError(e) => write!(fmt, "{:#?}", e),
            DHTError::LibP2PAddProviderError(e) => write!(fmt, "{:#?}", e),
            DHTError::LibP2PGetProvidersError(e) => write!(fmt, "{:#?}", e),
            DHTError::IO(k) => write!(fmt, "{:#?}", k),
            DHTError::Error(m) => write!(fmt, "{:#?}", m),
        }
    }
}

impl From<ChannelError> for DHTError {
    fn from(e: ChannelError) -> Self {
        match e {
            ChannelError::RecvError => DHTError::Error("RecvError".into()),
            ChannelError::SendError => DHTError::Error("SendError".into()),
        }
    }
}

impl From<anyhow::Error> for DHTError {
    fn from(e: anyhow::Error) -> Self {
        DHTError::Error(e.to_string())
    }
}

impl From<io::Error> for DHTError {
    fn from(e: io::Error) -> Self {
        DHTError::IO(e.kind())
    }
}

impl<TErr> From<TransportError<TErr>> for DHTError {
    fn from(e: TransportError<TErr>) -> Self {
        match e {
            TransportError::MultiaddrNotSupported(addr) => {
                DHTError::LibP2PTransportError(Some(addr))
            }
            TransportError::Other(_) => DHTError::LibP2PTransportError(None),
        }
    }
}

impl From<KadStorageError> for DHTError {
    fn from(e: KadStorageError) -> Self {
        DHTError::LibP2PStorageError(e)
    }
}

impl From<kad::GetRecordError> for DHTError {
    fn from(e: kad::GetRecordError) -> Self {
        DHTError::LibP2PGetRecordError(e)
    }
}

impl From<kad::PutRecordError> for DHTError {
    fn from(e: kad::PutRecordError) -> Self {
        DHTError::LibP2PPutRecordError(e)
    }
}

impl From<kad::BootstrapError> for DHTError {
    fn from(e: kad::BootstrapError) -> Self {
        DHTError::LibP2PBootstrapError(e)
    }
}

impl From<kad::AddProviderError> for DHTError {
    fn from(e: kad::AddProviderError) -> Self {
        DHTError::LibP2PAddProviderError(e)
    }
}

impl From<kad::GetProvidersError> for DHTError {
    fn from(e: kad::GetProvidersError) -> Self {
        DHTError::LibP2PGetProvidersError(e)
    }
}
