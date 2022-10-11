use crate::dht::types::DHTConfig;
use libp2p::kad;
use libp2p::kad::{Kademlia, KademliaConfig, KademliaEvent};
use libp2p::{NetworkBehaviour, PeerId};
use std::time::Duration;

const BOOTNODES: [&str; 4] = [
    "QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN",
    "QmQCU2EcMqAqQPR2i9bChDtGNJchTbq5TbXJJ16u19uLTa",
    "QmbLHAnMoJPWSCR5Zhtx6BHJX9KiKNN6tpvbUcqanj75Nb",
    "QmcZf59bWwK5XFi76CZX8cbJ4BhTzzA3gU1ZjYZcYW3dwt",
];

pub type DHTSwarm = libp2p::swarm::Swarm<DHTBehaviour>;

#[derive(Debug)]
pub enum DHTEvent {
    Kademlia(KademliaEvent),
    //Identify(Box<IdentifyEvent>),
}

impl From<KademliaEvent> for DHTEvent {
    fn from(event: KademliaEvent) -> Self {
        DHTEvent::Kademlia(event)
    }
}

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "DHTEvent")]
pub struct DHTBehaviour {
    //identify: Identify,
    pub kad: Kademlia<kad::record::store::MemoryStore>,
}

impl DHTBehaviour {
    pub fn new(config: &DHTConfig, local_peer_id: PeerId) -> Self {
        let mut cfg = KademliaConfig::default();
        cfg.set_query_timeout(Duration::from_secs(config.query_timeout.into()));
        // @TODO Use noosphere-fs instead of in-memory store.
        let store = kad::record::store::MemoryStore::new(local_peer_id);
        let kad = Kademlia::with_config(local_peer_id, store, cfg);
        DHTBehaviour { kad }
    }
}
