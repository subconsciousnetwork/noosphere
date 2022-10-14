use libp2p;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DHTBaseProtocol {
    Memory,
    IPv4,
    IPv6,
    Other,
}

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
    pub fn bootstrap_interval(mut self, interval: u64) -> Self {
        self.bootstrap_interval = interval;
        self
    }

    pub fn bootstrap_peers(mut self, peers: Vec<libp2p::Multiaddr>) -> Self {
        self.bootstrap_peers = peers;
        self
    }

    pub fn keypair(mut self, keypair: libp2p::identity::Keypair) -> Self {
        self.keypair = keypair;
        self
    }

    pub fn listening_address(mut self, address: libp2p::Multiaddr) -> Self {
        self.listening_address = address;
        self
    }

    pub fn peer_dialing_interval(mut self, interval: u64) -> Self {
        self.peer_dialing_interval = interval;
        self
    }

    pub fn query_timeout(mut self, timeout: u32) -> Self {
        self.query_timeout = timeout;
        self
    }

    // @TODO Cache this
    pub fn peer_id(&self) -> libp2p::PeerId {
        //utils::peer_id_from_key_with_sha256(&config.keypair.public())?
        libp2p::PeerId::from(self.keypair.public())
    }

    /// Computes the remote multiaddress of this node.
    /// Takes the listener address and appends the PeerId
    /// via the "p2p" protocol.
    /// Used only in tests for now.
    pub fn p2p_address(&self) -> libp2p::Multiaddr {
        let mut addr = self.listening_address.clone();
        addr.push(libp2p::multiaddr::Protocol::P2p(self.peer_id().into()));
        addr
    }

    /// Returns the base protocol used in listening address, e.g.
    /// "/ip4/123.12.3.123/tcp/1234" => DHTBaseProtocol::Ip4
    /// "/memory/0x12341234" => DHTBaseProtocol::Memory
    pub(crate) fn get_listening_base_transfer_protocol(&self) -> DHTBaseProtocol {
        let components = self
            .listening_address
            .iter()
            .collect::<Vec<libp2p::multiaddr::Protocol>>();
        if components.len() >= 1 {
            match components[0] {
                libp2p::multiaddr::Protocol::Memory(_) => DHTBaseProtocol::Memory,
                libp2p::multiaddr::Protocol::Ip4(_) => DHTBaseProtocol::IPv4,
                libp2p::multiaddr::Protocol::Ip6(_) => DHTBaseProtocol::IPv6,
                _ => DHTBaseProtocol::Other,
            }
        } else {
            DHTBaseProtocol::Other
        }
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
                .expect("default listening address is parseable."),
            peer_dialing_interval: 5,
            query_timeout: 5 * 60,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dht::DHTError;
    use libp2p::multiaddr::Protocol;
    use std::error::Error;
    use std::str::FromStr;

    #[test]
    fn test_dhtconfig_get_listening_base_transfer_protocol() -> Result<(), DHTError> {
        let expectations: Vec<(&str, DHTBaseProtocol)> = vec![
            (
                "/memory/4121452487836977756",
                DHTBaseProtocol::Memory,
            ),
            (
                "/memory/4121452487836977756/p2p/12D3KooWKxaeeDwM1151DRUXpa68pAi5yyRLn1gtWXvRqzWJG6rH",
                DHTBaseProtocol::Memory,
            ),
            ("/ip4/123.123.40.1", DHTBaseProtocol::IPv4),
            (
                "/ip6/2001:0db8:0000:0000:0000:8a2e:0370:7334",
                DHTBaseProtocol::IPv6,
            ),
            (
                "/dnsaddr/subconscious.network/tcp/4000",
                DHTBaseProtocol::Other,
            ),
            (
                "/dnsaddr/subconscious.network/tcp/4000",
                DHTBaseProtocol::Other,
            ),
        ];

        for expectation in expectations {
            let listening_address = libp2p::Multiaddr::from_str(expectation.0).unwrap();
            let config = DHTConfig::default().listening_address(listening_address.clone());
            let protocol = config.get_listening_base_transfer_protocol();
            let expected_protocol = expectation.1;
            assert_eq!(
                protocol, expected_protocol,
                "Expected {:?} for {:?}, got {:?}",
                expected_protocol, listening_address, protocol
            );
        }
        Ok(())
    }

    #[test]
    fn test_dhtconfig_p2p_address() -> Result<(), Box<dyn Error>> {
        let config = DHTConfig::default()
            .listening_address("/ip4/127.0.0.1/tcp/0".parse::<libp2p::Multiaddr>()?);
        let mut address = config.p2p_address();
        assert_eq!(
            address.pop().unwrap(),
            Protocol::P2p(config.peer_id().into())
        );
        assert_eq!(address.pop().unwrap(), Protocol::Tcp(0));
        assert_eq!(
            address.pop().unwrap(),
            Protocol::Ip4("127.0.0.1".parse().unwrap())
        );
        Ok(())
    }
}
