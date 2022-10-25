use libp2p;

#[derive(Clone, Debug)]
pub struct DHTConfig {
    /// If bootstrap peers are provided, how often,
    /// in seconds, should the bootstrap process execute
    /// to keep routing tables fresh.
    pub bootstrap_interval: u64,
    /// Peer addresses to query to update routing tables
    /// during bootstrap. A standalone bootstrap node would
    /// have this field empty.
    pub bootstrap_peers: Vec<libp2p::Multiaddr>,
    pub keypair: libp2p::identity::Keypair,
    /// Address to listen for incoming connections.
    pub listening_address: libp2p::Multiaddr,
    /// How frequently, in seconds, the DHT attempts to
    /// dial peers found in its kbucket. Outside of tests,
    /// should not be lower than 5 seconds.
    pub peer_dialing_interval: u64,
    pub query_timeout: u32,
}

impl DHTConfig {
    /// Computes the [libp2p::PeerId] and [libp2p::Multiaddr]
    /// listening address from the provided [DHTConfig].
    pub fn get_peer_id_and_address(config: &DHTConfig) -> (libp2p::PeerId, libp2p::Multiaddr) {
        let peer_id = libp2p::PeerId::from(config.keypair.public());
        let mut addr = config.listening_address.clone();
        addr.push(libp2p::multiaddr::Protocol::P2p(peer_id.into()));
        (peer_id, addr)
    }
}

impl Default for DHTConfig {
    fn default() -> Self {
        Self {
            bootstrap_interval: 5 * 60,
            bootstrap_peers: vec![],
            keypair: libp2p::identity::Keypair::generate_ed25519(),
            listening_address: "/ip4/127.0.0.1/tcp/0"
                .parse::<libp2p::Multiaddr>()
                .expect("Default address is parseable."),
            peer_dialing_interval: 5,
            query_timeout: 5 * 60,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::multiaddr::Protocol;
    use std::error::Error;

    #[test]
    fn test_dhtconfig_get_peer_id_and_address() -> Result<(), Box<dyn Error>> {
        let mut config = DHTConfig::default();
        config.listening_address = "/ip4/127.0.0.50/tcp/33333".parse::<libp2p::Multiaddr>()?;
        let keypair = &config.keypair;
        let (peer_id, mut address) = DHTConfig::get_peer_id_and_address(&config);

        assert_eq!(peer_id, libp2p::PeerId::from(keypair.public()));
        assert_eq!(address.pop().unwrap(), Protocol::P2p(peer_id.into()));
        assert_eq!(address.pop().unwrap(), Protocol::Tcp(33333));
        assert_eq!(
            address.pop().unwrap(),
            Protocol::Ip4("127.0.0.50".parse().unwrap())
        );
        Ok(())
    }
}
