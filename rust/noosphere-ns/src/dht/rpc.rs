use crate::dht::channel::{Message, MessageClient, MessageProcessor};
use crate::dht::errors::DHTError;
use libp2p::{swarm::NetworkInfo, Multiaddr};

use std::{fmt, str};

#[derive(Debug, PartialEq, Eq)]
pub struct DHTNetworkInfo {
    pub num_peers: usize,
    pub num_connections: u32,
    pub num_pending: u32,
    pub num_established: u32,
}

impl From<NetworkInfo> for DHTNetworkInfo {
    fn from(info: NetworkInfo) -> Self {
        let c = info.connection_counters();
        DHTNetworkInfo {
            num_peers: info.num_peers(),
            num_connections: c.num_connections(),
            num_pending: c.num_pending(),
            num_established: c.num_established(),
        }
    }
}
#[derive(Debug, Clone)]
pub struct DHTRecord {
    pub key: Vec<u8>,
    pub value: Option<Vec<u8>>,
}

impl fmt::Display for DHTRecord {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = if let Some(value) = self.value.as_ref() {
            str::from_utf8(value)
        } else {
            Ok("None")
        };
        write!(
            fmt,
            "DHTRecord {{ key: {:?}, value: {:?} }}",
            str::from_utf8(&self.key),
            value
        )
    }
}

#[derive(Debug)]
pub enum DHTRequest {
    AddPeers { peers: Vec<Multiaddr> },
    StartListening { address: Multiaddr },
    StopListening { address: Multiaddr },
    Bootstrap,
    //WaitForPeers(usize),
    GetAddresses { external: bool },
    GetNetworkInfo,
    GetRecord { key: Vec<u8> },
    PutRecord { key: Vec<u8>, value: Vec<u8> },
    StartProviding { key: Vec<u8> },
    GetProviders { key: Vec<u8> },
}

impl fmt::Display for DHTRequest {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            //DHTRequest::WaitForPeers(peers) => write!(fmt, "DHTRequest::WaitForPeers({})", peers),
            DHTRequest::AddPeers { peers } => {
                write!(fmt, "DHTRequest::AddPeers {{ peers={:?} }}", peers)
            }
            DHTRequest::StartListening { address } => {
                write!(
                    fmt,
                    "DHTRequest::StartListening {{ address={:?} }}",
                    address
                )
            }
            DHTRequest::StopListening { address } => {
                write!(fmt, "DHTRequest::StopListening {{ address={:?} }}", address)
            }
            DHTRequest::Bootstrap => write!(fmt, "DHTRequest::Bootstrap"),
            DHTRequest::GetAddresses { external } => write!(
                fmt,
                "DHTRequest::GetAddresses {{ external={:?} }}",
                external
            ),
            DHTRequest::GetNetworkInfo => write!(fmt, "DHTRequest::GetNetworkInfo"),
            DHTRequest::GetRecord { key } => write!(
                fmt,
                "DHTRequest::GetRecord {{ key={:?} }}",
                str::from_utf8(key)
            ),
            DHTRequest::PutRecord { key, value } => write!(
                fmt,
                "DHTRequest::PutRecord {{ key={:?}, value={:?} }}",
                str::from_utf8(key),
                str::from_utf8(value)
            ),
            DHTRequest::StartProviding { key } => write!(
                fmt,
                "DHTRequest::StartProviding {{ key={:?} }}",
                str::from_utf8(key)
            ),
            DHTRequest::GetProviders { key } => write!(
                fmt,
                "DHTRequest::GetProviders {{ key={:?} }}",
                str::from_utf8(key)
            ),
        }
    }
}

#[derive(Debug)]
pub enum DHTResponse {
    Success,
    GetAddresses(Vec<Multiaddr>),
    GetNetworkInfo(DHTNetworkInfo),
    GetRecord(DHTRecord),
    PutRecord { key: Vec<u8> },
    GetProviders { providers: Vec<libp2p::PeerId> },
}

impl fmt::Display for DHTResponse {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DHTResponse::Success => write!(fmt, "DHTResponse::Success"),
            DHTResponse::GetAddresses(addresses) => {
                write!(fmt, "DHTResponse::GetAddresses {{ {:?} }}", addresses)
            }
            DHTResponse::GetNetworkInfo(info) => {
                write!(fmt, "DHTResponse::GetNetworkInfo {:?}", info)
            }
            DHTResponse::GetRecord(record) => {
                write!(fmt, "DHTResponse::GetRecord {{ {:?} }}", record)
            }
            DHTResponse::PutRecord { key } => write!(
                fmt,
                "DHTResponse::PutRecord {{ key={:?} }}",
                str::from_utf8(key)
            ),
            DHTResponse::GetProviders { providers } => write!(
                fmt,
                "DHTResponse::GetProviders {{ providers={:?} }}",
                providers
            ),
        }
    }
}

pub type DHTMessage = Message<DHTRequest, DHTResponse, DHTError>;
pub type DHTMessageProcessor = MessageProcessor<DHTRequest, DHTResponse, DHTError>;
pub type DHTMessageClient = MessageClient<DHTRequest, DHTResponse, DHTError>;
