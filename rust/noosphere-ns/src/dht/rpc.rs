use crate::dht::channel::{Message, MessageClient, MessageProcessor};
use crate::dht::errors::DhtError;
use crate::dht::types::{DhtRecord, NetworkInfo, Peer};
use libp2p::Multiaddr;

use std::{fmt, str};

#[derive(Debug)]
pub enum DhtRequest {
    AddPeers {
        peers: Vec<Multiaddr>,
    },
    StartListening {
        address: Multiaddr,
    },
    StopListening,
    Bootstrap,
    //WaitForPeers(usize),
    GetAddresses {
        external: bool,
    },
    GetPeers,
    GetNetworkInfo,
    GetRecord {
        key: Vec<u8>,
    },
    PutRecord {
        key: Vec<u8>,
        value: Vec<u8>,
        quorum: usize,
    },
    StartProviding {
        key: Vec<u8>,
    },
    GetProviders {
        key: Vec<u8>,
    },
}

impl fmt::Display for DhtRequest {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            //DHTRequest::WaitForPeers(peers) => write!(fmt, "DHTRequest::WaitForPeers({})", peers),
            DhtRequest::AddPeers { peers } => {
                write!(fmt, "DHTRequest::AddPeers {{ peers={:?} }}", peers)
            }
            DhtRequest::StartListening { address } => {
                write!(
                    fmt,
                    "DHTRequest::StartListening {{ address={:?} }}",
                    address
                )
            }
            DhtRequest::StopListening => {
                write!(fmt, "DHTRequest::StopListening")
            }
            DhtRequest::Bootstrap => write!(fmt, "DHTRequest::Bootstrap"),
            DhtRequest::GetAddresses { external } => write!(
                fmt,
                "DHTRequest::GetAddresses {{ external={:?} }}",
                external
            ),
            DhtRequest::GetPeers => write!(fmt, "DHTRequest::GetPeers"),
            DhtRequest::GetNetworkInfo => write!(fmt, "DHTRequest::GetNetworkInfo"),
            DhtRequest::GetRecord { key } => write!(
                fmt,
                "DHTRequest::GetRecord {{ key={:?} }}",
                str::from_utf8(key)
            ),
            DhtRequest::PutRecord { key, value, quorum } => write!(
                fmt,
                "DHTRequest::PutRecord {{ key={:?}, value={:?}, quorum={:?} }}",
                str::from_utf8(key),
                str::from_utf8(value),
                quorum,
            ),
            DhtRequest::StartProviding { key } => write!(
                fmt,
                "DHTRequest::StartProviding {{ key={:?} }}",
                str::from_utf8(key)
            ),
            DhtRequest::GetProviders { key } => write!(
                fmt,
                "DHTRequest::GetProviders {{ key={:?} }}",
                str::from_utf8(key)
            ),
        }
    }
}

#[derive(Debug)]
pub enum DhtResponse {
    Success,
    Address(Multiaddr),
    GetAddresses(Vec<Multiaddr>),
    GetNetworkInfo(NetworkInfo),
    GetPeers(Vec<Peer>),
    GetRecord(DhtRecord),
    PutRecord { key: Vec<u8> },
    GetProviders { providers: Vec<libp2p::PeerId> },
}

impl fmt::Display for DhtResponse {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DhtResponse::Success => write!(fmt, "DHTResponse::Success"),
            DhtResponse::Address(address) => {
                write!(fmt, "DHTResponse::Address {{ {:?} }}", address)
            }
            DhtResponse::GetAddresses(addresses) => {
                write!(fmt, "DHTResponse::GetAddresses {{ {:?} }}", addresses)
            }
            DhtResponse::GetPeers(peers) => {
                write!(fmt, "DHTResponse::GetPeers{:?}", peers)
            }
            DhtResponse::GetNetworkInfo(info) => {
                write!(fmt, "DHTResponse::GetNetworkInfo {:?}", info)
            }
            DhtResponse::GetRecord(record) => {
                write!(fmt, "DHTResponse::GetRecord {{ {:?} }}", record)
            }
            DhtResponse::PutRecord { key } => write!(
                fmt,
                "DHTResponse::PutRecord {{ key={:?} }}",
                str::from_utf8(key)
            ),
            DhtResponse::GetProviders { providers } => write!(
                fmt,
                "DHTResponse::GetProviders {{ providers={:?} }}",
                providers
            ),
        }
    }
}

pub type DhtMessage = Message<DhtRequest, DhtResponse, DhtError>;
pub type DhtMessageProcessor = MessageProcessor<DhtRequest, DhtResponse, DhtError>;
pub type DhtMessageClient = MessageClient<DhtRequest, DhtResponse, DhtError>;
