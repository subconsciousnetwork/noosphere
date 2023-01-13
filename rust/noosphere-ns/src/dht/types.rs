use libp2p::{swarm::NetworkInfo as LibP2pNetworkInfo, PeerId};
use serde::{Deserialize, Serialize};

use std::{fmt, str};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub num_peers: usize,
    pub num_connections: u32,
    pub num_pending: u32,
    pub num_established: u32,
}

impl From<LibP2pNetworkInfo> for NetworkInfo {
    fn from(info: LibP2pNetworkInfo) -> Self {
        let c = info.connection_counters();
        NetworkInfo {
            num_peers: info.num_peers(),
            num_connections: c.num_connections(),
            num_pending: c.num_pending(),
            num_established: c.num_established(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize, Clone)]
pub struct Peer {
    pub peer_id: PeerId,
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
