use crate::dht::errors::DHTError;
use crate::dht::utils;
use crate::NameSystemConfig;
use libp2p;
use std::result::Result;

/*
const DEFAULT_BOOTSTRAP_PEERS: &[&str] =
    //&["/ip4/134.122.20.28/tcp/6666/p2p/QmYbGzVB6L6EcAWkyxZhtR2Yd9VekqjmuUSkLmAiLJxhtF"];
    //&["/ip4/134.122.20.28/tcp/6666/p2p/12D3KooWGp95tnFDu6fBMAW4hYXZUCVgkPceJzUiEEejMXB5Zk4g"];
    &["/ip4/127.0.0.1/tcp/6666/p2p/12D3KooWGp95tnFDu6fBMAW4hYXZUCVgkPceJzUiEEejMXB5Zk4g"];
        let bootstrap_peers = DEFAULT_BOOTSTRAP_PEERS
            .iter()
            .map(|node| node.parse::<libp2p::Multiaddr>().unwrap())
            .collect();
 */

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DHTBaseProtocol {
    Memory,
    IPv4,
    IPv6,
    Other,
}

#[derive(Clone, Debug)]
pub struct DHTConfig {
    pub keypair: libp2p::identity::Keypair,
    pub query_timeout: u32,
    /// Peer addresses to query to update routing tables
    /// during bootstrap. A standalone bootstrap node would
    /// have this field empty.
    pub bootstrap_peers: Vec<libp2p::Multiaddr>,
    /// Address to listen for incoming connections.
    /// Only for server-nodes/peers.
    pub listening_address: Option<libp2p::Multiaddr>,
}

impl DHTConfig {
    pub fn peer_id(&self) -> libp2p::PeerId {
        //utils::peer_id_from_key_with_sha256(&config.keypair.public())?
        libp2p::PeerId::from(self.keypair.public())
    }

    pub fn is_server(self) -> bool {
        self.listening_address.is_some()
    }

    /// Returns the base protocol used in listening address, e.g.
    /// "/ip4/123.12.3.123/tcp/1234" => DHTBaseProtocol::Ip4
    /// "/memory/0x12341234" => DHTBaseProtocol::Memory
    pub(crate) fn get_listening_base_transfer_protocol(&self) -> Option<DHTBaseProtocol> {
        match &self.listening_address {
            Some(addr) => {
                let components = addr.iter().collect::<Vec<libp2p::multiaddr::Protocol>>();
                if components.len() >= 1 {
                    match components[0] {
                        libp2p::multiaddr::Protocol::Memory(_) => Some(DHTBaseProtocol::Memory),
                        libp2p::multiaddr::Protocol::Ip4(_) => Some(DHTBaseProtocol::IPv4),
                        libp2p::multiaddr::Protocol::Ip6(_) => Some(DHTBaseProtocol::IPv6),
                        _ => Some(DHTBaseProtocol::Other),
                    }
                } else {
                    None
                }
            }
            None => None,
        }
    }
}

impl<'a> TryFrom<NameSystemConfig<'a>> for DHTConfig {
    type Error = DHTError;
    fn try_from(config: NameSystemConfig<'a>) -> Result<Self, Self::Error> {
        let mut dht_config = DHTConfig {
            query_timeout: config.query_timeout,
            ..Default::default()
        };
        if let Some(key_material) = config.key_material {
            dht_config.keypair = utils::key_material_to_libp2p_keypair(key_material)
                .map_err(|e| DHTError::from(e))?
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
        Self {
            keypair: libp2p::identity::Keypair::Ed25519(
                libp2p::identity::ed25519::Keypair::generate(),
            ),
            query_timeout: 5 * 60,
            listening_address: None,
            bootstrap_peers: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    #[test]
    fn test_dhtconfig_get_listening_base_transfer_protocol() -> Result<(), DHTError> {
        let expectations: Vec<(Option<&str>, Option<DHTBaseProtocol>)> = vec![
            (
                Some("/memory/4121452487836977756"),
                Some(DHTBaseProtocol::Memory),
            ),
            (Some("/ip4/123.123.40.1"), Some(DHTBaseProtocol::IPv4)),
            (
                Some("/ip6/2001:0db8:0000:0000:0000:8a2e:0370:7334"),
                Some(DHTBaseProtocol::IPv6),
            ),
            (
                Some("/dnsaddr/subconscious.network/tcp/4000"),
                Some(DHTBaseProtocol::Other),
            ),
            (
                Some("/dnsaddr/subconscious.network/tcp/4000"),
                Some(DHTBaseProtocol::Other),
            ),
            (None, None),
        ];

        for expectation in expectations {
            let listening_address = match expectation.0 {
                Some(s) => Some(libp2p::Multiaddr::from_str(s).unwrap()),
                None => None,
            };
            let config = DHTConfig {
                listening_address: listening_address.clone(),
                ..Default::default()
            };
            let protocol = config.get_listening_base_transfer_protocol();
            match expectation.1 {
                Some(expected_protocol) => {
                    assert!(
                        protocol.is_some(),
                        "Expected Some for {:?}",
                        listening_address
                    );
                    let p = protocol.unwrap();
                    assert_eq!(
                        p, expected_protocol,
                        "Expected {:?} for {:?}, got {:?}",
                        expected_protocol, listening_address, p
                    );
                }
                None => assert!(
                    protocol.is_none(),
                    "Expected None for {:?}, got Some",
                    listening_address
                ),
            }
        }
        Ok(())
    }
}
