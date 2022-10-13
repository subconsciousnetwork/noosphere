use crate::dht::DHTConfig;
use libp2p::kad::{Kademlia, KademliaConfig, KademliaEvent};
use libp2p::{kad, multiaddr};
use libp2p::{NetworkBehaviour, PeerId};
use std::time::Duration;

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
        let kad = {
            let mut cfg = KademliaConfig::default();
            cfg.set_query_timeout(Duration::from_secs(config.query_timeout.into()));

            // @TODO Use noosphere-fs instead of in-memory store.
            let store = kad::record::store::MemoryStore::new(local_peer_id);
            let mut kad = Kademlia::with_config(local_peer_id, store, cfg);

            // Add the bootnodes to the local routing table.
            for multiaddress in &config.bootstrap_peers {
                let mut addr = multiaddress.to_owned();
                if let Some(multiaddr::Protocol::P2p(p2p_hash)) = addr.pop() {
                    let peer_id = PeerId::from_multihash(p2p_hash).unwrap();
                    if peer_id != local_peer_id {
                        trace!("Adding bootstrap peer {:#?}", multiaddress);
                        kad.add_address(&peer_id, addr);
                    }
                }
            }
            kad
        };
        DHTBehaviour { kad }
    }
}
