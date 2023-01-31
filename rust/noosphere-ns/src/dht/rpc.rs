use crate::dht::channel::{Message, MessageClient, MessageProcessor};
use crate::dht::errors::DHTError;
use crate::dht::types::{DHTRecord, NetworkInfo, Peer};
use libp2p::Multiaddr;

use std::{fmt, str};

#[derive(Debug)]
pub enum DHTRequest {
    AddPeers { peers: Vec<Multiaddr> },
    StartListening { address: Multiaddr },
    StopListening,
    Bootstrap,
    //WaitForPeers(usize),
    GetAddresses { external: bool },
    GetPeers,
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
            DHTRequest::StopListening => {
                write!(fmt, "DHTRequest::StopListening")
            }
            DHTRequest::Bootstrap => write!(fmt, "DHTRequest::Bootstrap"),
            DHTRequest::GetAddresses { external } => write!(
                fmt,
                "DHTRequest::GetAddresses {{ external={:?} }}",
                external
            ),
            DHTRequest::GetPeers => write!(fmt, "DHTRequest::GetPeers"),
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
    Address(Multiaddr),
    GetAddresses(Vec<Multiaddr>),
    GetNetworkInfo(NetworkInfo),
    GetPeers(Vec<Peer>),
    GetRecord(DHTRecord),
    PutRecord { key: Vec<u8> },
    GetProviders { providers: Vec<libp2p::PeerId> },
}

impl fmt::Display for DHTResponse {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DHTResponse::Success => write!(fmt, "DHTResponse::Success"),
            DHTResponse::Address(address) => {
                write!(fmt, "DHTResponse::Address {{ {:?} }}", address)
            }
            DHTResponse::GetAddresses(addresses) => {
                write!(fmt, "DHTResponse::GetAddresses {{ {:?} }}", addresses)
            }
            DHTResponse::GetPeers(peers) => {
                write!(fmt, "DHTResponse::GetPeers{:?}", peers)
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
