use crate::dht::{DHTConfig, DHTNode};
use anyhow::anyhow;
use libp2p;
use noosphere::authority::ed25519_key_to_bytes;
use ucan_key_support::ed25519::Ed25519KeyMaterial;

/// [DHTNodeBuilder] is the primary external interface for
/// creating a new [DHTNode].
///
/// # Examples
///
/// ```
/// use tokio;
/// use noosphere::authority::generate_ed25519_key;
/// use noosphere_p2p::dht::DHTNodeBuilder;
/// use anyhow::{Result, Error};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Error> {
///     let bootstrap_key = generate_ed25519_key();
///     let bootstrap = DHTNodeBuilder::default()
///         .listening_address("/ip4/127.0.0.1/tcp/30000")
///         .key_material(&bootstrap_key)
///         .build()?;
///
///     let bootstrap_address: String = bootstrap.p2p_address().to_string();
///     let bootstrap_peers = vec![bootstrap_address.as_str()];
///     let client_key = generate_ed25519_key();
///     let client_node = DHTNodeBuilder::default()
///         .listening_address("/ip4/127.0.0.1/tcp/20000")
///         .key_material(&client_key)
///         .bootstrap_peers(&bootstrap_peers)
///         .build()?;
///     Ok(())
/// }
/// ```
///
pub struct DHTNodeBuilder<'a> {
    bootstrap_interval: u64,
    bootstrap_peers: Option<&'a Vec<&'a str>>,
    key_material: Option<&'a Ed25519KeyMaterial>,
    listening_address: Option<&'a str>,
    peer_dialing_interval: u64,
    query_timeout: u32,
}

impl<'a> DHTNodeBuilder<'a> {
    /// If bootstrap peers are provided, how often,
    /// in seconds, should the bootstrap process execute
    /// to keep routing tables fresh.
    pub fn bootstrap_interval(mut self, interval: u64) -> Self {
        self.bootstrap_interval = interval;
        self
    }

    /// Peer addresses to query to update routing tables
    /// during bootstrap. A standalone bootstrap node would
    /// have this field empty.
    pub fn bootstrap_peers(mut self, peers: &'a Vec<&'a str>) -> Self {
        self.bootstrap_peers = Some(peers);
        self
    }

    /// Public/private keypair for DHT node.
    pub fn key_material(mut self, key_material: &'a Ed25519KeyMaterial) -> Self {
        self.key_material = Some(key_material);
        self
    }

    /// Address to listen for incoming connections.
    pub fn listening_address(mut self, address: &'a str) -> Self {
        self.listening_address = Some(address);
        self
    }

    /// How frequently, in seconds, the DHT attempts to
    /// dial peers found in its kbucket. Outside of tests,
    /// should not be lower than 5 seconds.
    pub fn peer_dialing_interval(mut self, interval: u64) -> Self {
        self.peer_dialing_interval = interval;
        self
    }

    pub fn query_timeout(mut self, timeout: u32) -> Self {
        self.query_timeout = timeout;
        self
    }

    pub fn build(self) -> Result<DHTNode, anyhow::Error> {
        let keypair = if let Some(km) = self.key_material {
            key_material_to_libp2p_keypair(km)?
        } else {
            libp2p::identity::Keypair::generate_ed25519()
        };

        let listening_address = if let Some(addr) = self.listening_address {
            addr.parse::<libp2p::Multiaddr>()?
        } else {
            "/ip4/127.0.0.1/tcp/0"
                .parse::<libp2p::Multiaddr>()
                .expect("default listening address is parseable.")
        };

        let bootstrap_peers: Vec<libp2p::Multiaddr> = if let Some(peers) = self.bootstrap_peers {
            peers
                .iter()
                .map(|s| s.parse::<libp2p::Multiaddr>())
                .collect::<Result<Vec<libp2p::Multiaddr>, _>>()
                .map_err(|e| anyhow!(e.to_string()))?
        } else {
            vec![]
        };

        DHTNode::new(DHTConfig {
            bootstrap_interval: self.bootstrap_interval,
            bootstrap_peers,
            keypair,
            listening_address,
            peer_dialing_interval: self.peer_dialing_interval,
            query_timeout: self.query_timeout,
        })
        .map_err(|e| anyhow!(e.to_string()))
    }
}

impl<'a> Default for DHTNodeBuilder<'a> {
    fn default() -> Self {
        Self {
            bootstrap_interval: 5 * 60,
            bootstrap_peers: None,
            key_material: None,
            listening_address: None,
            peer_dialing_interval: 5,
            query_timeout: 5 * 60,
        }
    }
}

pub fn key_material_to_libp2p_keypair(
    key_material: &Ed25519KeyMaterial,
) -> Result<libp2p::identity::Keypair, anyhow::Error> {
    let mut bytes = ed25519_key_to_bytes(key_material)?;
    let kp = libp2p::identity::ed25519::Keypair::decode(&mut bytes)
        .map_err(|_| anyhow!("Could not decode ED25519 key."))?;
    Ok(libp2p::identity::Keypair::Ed25519(kp))
}

#[cfg(test)]
mod tests {
    use super::*;
    use noosphere::authority::generate_ed25519_key;
    use tokio;

    #[test]
    fn test_key_material_to_libp2p_keypair() -> Result<(), anyhow::Error> {
        let zebra_keys = generate_ed25519_key();
        let keypair: libp2p::identity::ed25519::Keypair =
            match key_material_to_libp2p_keypair(&zebra_keys) {
                Ok(kp) => match kp {
                    libp2p::identity::Keypair::Ed25519(keypair) => Ok(keypair),
                },
                Err(e) => Err(e),
            }?;
        let zebra_private_key = zebra_keys.1.expect("Has private key");
        let dalek_public_key = keypair.public().encode();
        let dalek_private_key = keypair.secret();

        let in_public_key = zebra_keys.0.as_ref();
        let in_private_key = zebra_private_key.as_ref();
        let out_public_key = dalek_public_key.as_ref();
        let out_private_key = dalek_private_key.as_ref();
        assert_eq!(in_public_key, out_public_key);
        assert_eq!(in_private_key, out_private_key);
        Ok(())
    }

    #[tokio::test]
    async fn test_dht_node_builder() -> Result<(), anyhow::Error> {
        let key_material = generate_ed25519_key();
        let expected_libp2p_keypair = key_material_to_libp2p_keypair(&key_material)?;
        let expected_peer_id = libp2p::PeerId::from(expected_libp2p_keypair.public());
        let bootstrap_peers = vec![
            "/ip4/127.0.0.50/tcp/33333/p2p/12D3KooWH8WgH9mgbMXrKX4veokUznvEn6Ycwg4qaGNi83nLkoUK",
            "/ip4/127.0.0.50/tcp/33334/p2p/12D3KooWMWo6tNGRx1G4TNqvr4SnHyVXSReC3tdX6zoJothXxV2c",
        ];
        let listening_address = "/ip4/10.0.0.1/tcp/12000";

        let node = DHTNodeBuilder::default()
            .listening_address(listening_address)
            .key_material(&key_material)
            .bootstrap_peers(&bootstrap_peers)
            .bootstrap_interval(33)
            .peer_dialing_interval(11)
            .query_timeout(22)
            .build()?;

        let config = node.config();
        assert_eq!(
            config.listening_address,
            listening_address.parse::<libp2p::Multiaddr>().unwrap()
        );
        assert_eq!(node.peer_id(), &expected_peer_id);
        assert_eq!(config.keypair.public(), expected_libp2p_keypair.public());
        assert_eq!(config.bootstrap_peers.len(), 2);
        assert_eq!(
            config.bootstrap_peers[0],
            bootstrap_peers[0].parse::<libp2p::Multiaddr>().unwrap()
        );
        assert_eq!(
            config.bootstrap_peers[1],
            bootstrap_peers[1].parse::<libp2p::Multiaddr>().unwrap()
        );
        assert_eq!(config.bootstrap_interval, 33);
        assert_eq!(config.peer_dialing_interval, 11);
        assert_eq!(config.query_timeout, 22);
        Ok(())
    }
}
