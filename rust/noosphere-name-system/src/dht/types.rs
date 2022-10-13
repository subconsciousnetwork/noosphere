use crate::dht::channel::{Message, MessageClient, MessageProcessor};
use crate::dht::errors::DHTError;
use libp2p::swarm::NetworkInfo;
use std::{fmt, str};

#[derive(Clone, PartialEq, Debug)]
pub enum DHTStatus {
    Inactive,
    Active,
    Error(String),
}

#[derive(Debug)]
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

#[derive(Debug)]
pub enum DHTRequest {
    Bootstrap,
    GetNetworkInfo,
    GetRecord { name: Vec<u8> },
    SetRecord { name: Vec<u8>, value: Vec<u8> },
    StartProviding { name: Vec<u8> },
}

impl fmt::Display for DHTRequest {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DHTRequest::Bootstrap => write!(fmt, "DHTRequest::Bootstrap"),
            DHTRequest::GetNetworkInfo => write!(fmt, "DHTRequest::GetNetworkInfo"),
            DHTRequest::GetRecord { name } => write!(
                fmt,
                "DHTRequest::GetRecord {{ name={:?} }}",
                str::from_utf8(name)
            ),
            DHTRequest::SetRecord { name, value } => write!(
                fmt,
                "DHTRequest::SetRecord {{ name={:?}, value={:?} }}",
                str::from_utf8(name),
                str::from_utf8(value)
            ),
            DHTRequest::StartProviding { name } => write!(
                fmt,
                "DHTRequest::StartProviding {{ name={:?} }}",
                str::from_utf8(name)
            ),
        }
    }
}

#[derive(Debug)]
pub enum DHTResponse {
    Bootstrap(DHTNetworkInfo),
    GetNetworkInfo(DHTNetworkInfo),
    GetRecord { name: Vec<u8>, value: Vec<u8> },
    SetRecord { name: Vec<u8> },
    StartProviding { name: Vec<u8> },
}

impl fmt::Display for DHTResponse {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DHTResponse::Bootstrap(info) => write!(fmt, "DHTResponse::Bootstrap {:?}", info),
            DHTResponse::GetNetworkInfo(info) => {
                write!(fmt, "DHTResponse::GetNetworkInfo {:?}", info)
            }
            DHTResponse::GetRecord { name, value } => write!(
                fmt,
                "DHTResponse::GetRecord {{ name={:?}, value={:?} }}",
                str::from_utf8(name),
                str::from_utf8(value)
            ),
            DHTResponse::SetRecord { name } => write!(
                fmt,
                "DHTResponse::SetRecord {{ name={:?} }}",
                str::from_utf8(name)
            ),
            DHTResponse::StartProviding { name } => write!(
                fmt,
                "DHTResponse::StartProviding {{ name={:?} }}",
                str::from_utf8(name)
            ),
        }
    }
}

pub type DHTMessage = Message<DHTRequest, DHTResponse, DHTError>;
pub type DHTMessageProcessor = MessageProcessor<DHTRequest, DHTResponse, DHTError>;
pub type DHTMessageClient = MessageClient<DHTRequest, DHTResponse, DHTError>;
