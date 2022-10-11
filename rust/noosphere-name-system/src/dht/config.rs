use crate::dht::utils;
use crate::NameSystemConfig;
use anyhow::{Error, Result};
use libp2p;

const DEFAULT_BOOTSTRAP_PEERS: &[&str] =
    &["/ip4/134.122.20.28/tcp/6666/p2p/QmYbGzVB6L6EcAWkyxZhtR2Yd9VekqjmuUSkLmAiLJxhtF"];

#[derive(Clone, Debug)]
pub struct DHTConfig {
    pub keypair: libp2p::identity::Keypair,
    pub query_timeout: u32,
    pub bootstrap_peers: Vec<libp2p::Multiaddr>,
    /// Address to listen for incoming connections.
    /// Only for server-nodes/peers.
    pub listening_address: Option<libp2p::Multiaddr>,

    peer_id: Option<libp2p::PeerId>,
}

impl DHTConfig {
    /// Returns a reference to the [libp2p::PeerId] representing
    /// the corresponding [libp2p::identity::Keypair].
    pub fn peer_id(&mut self) -> Result<&libp2p::PeerId> {
        if self.peer_id.is_none() {
            self.peer_id = Some(libp2p::PeerId::from(self.keypair.public()));
            // self.peer_id = Some(utils::peer_id_from_key_with_sha256(&self.keypair.public())?);
        }
        Ok(self.peer_id.as_ref().unwrap())
    }
}

impl<'a> TryFrom<NameSystemConfig<'a>> for DHTConfig {
    type Error = Error;
    fn try_from(config: NameSystemConfig<'a>) -> Result<Self, Self::Error> {
        let mut dht_config = DHTConfig {
            query_timeout: config.query_timeout,
            ..Default::default()
        };
        if let Some(key_material) = config.key_material {
            dht_config.keypair = utils::key_material_to_libp2p_keypair(key_material)?
        }
        if let Some(server_port) = config.server_port {
            // Hardcode listening ip4 address for now.
            let mut address: libp2p::Multiaddr = "/ip4/127.0.0.1".parse().unwrap();
            address.push(libp2p::multiaddr::Protocol::Tcp(server_port));
            dht_config.listening_address = Some(address);
        }
        if let Some(bootstrap_peers) = config.bootstrap_peers {
            let peers: Vec<libp2p::Multiaddr> = bootstrap_peers
                .into_iter()
                .filter_map(|addr| match libp2p::Multiaddr::try_from(addr) {
                    // @TODO ignore bootstrap nodes that match current node PeerId
                    Ok(parsed) => Some(parsed),
                    Err(e) => {
                        warn!("{}", e);
                        None
                    }
                })
                .collect();
            dht_config.bootstrap_peers = peers;
        }
        Ok(dht_config)
    }
}

impl Default for DHTConfig {
    fn default() -> Self {
        debug!("BOOTSTRAP: {:#?}", DEFAULT_BOOTSTRAP_PEERS);
        let bootstrap_peers = DEFAULT_BOOTSTRAP_PEERS
            .iter()
            .map(|node| node.parse::<libp2p::Multiaddr>().unwrap())
            .collect();

        Self {
            keypair: libp2p::identity::Keypair::Ed25519(
                libp2p::identity::ed25519::Keypair::generate(),
            ),
            query_timeout: 5 * 60,
            listening_address: None,
            bootstrap_peers,
            peer_id: None,
        }
    }
}
