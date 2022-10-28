use crate::{dht::DHTConfig, name_system::NameSystem};
use anyhow::{anyhow, Result};
use libp2p::{self, Multiaddr};
use std::collections::HashMap;
use std::net::Ipv4Addr;
use ucan_key_support::ed25519::Ed25519KeyMaterial;

/// [NameSystemBuilder] is the primary external interface for
/// creating a new [NameSystem].
///
/// # Examples
///
/// ```
/// use noosphere_core::authority::generate_ed25519_key;
/// use noosphere_ns::{NameSystem, NameSystemBuilder};
///
/// let key_material = generate_ed25519_key();
/// let ns = NameSystemBuilder::default()
///     .key_material(&key_material)
///     .listening_port(30000)
///     .build().expect("valid config");
///
/// ```
pub struct NameSystemBuilder<'a> {
    bootstrap_peers: Option<&'a Vec<Multiaddr>>,
    dht_config: DHTConfig,
    key_material: Option<&'a Ed25519KeyMaterial>,
    ttl: u64,
}

impl<'a> NameSystemBuilder<'a> {
    /// If bootstrap peers are provided, how often,
    /// in seconds, should the bootstrap process execute
    /// to keep routing tables fresh.
    pub fn bootstrap_interval(mut self, interval: u64) -> Self {
        self.dht_config.bootstrap_interval = interval;
        self
    }

    /// Peer addresses to query to update routing tables
    /// during bootstrap. A standalone bootstrap node would
    /// have this field empty.
    pub fn bootstrap_peers(mut self, peers: &'a Vec<Multiaddr>) -> Self {
        self.bootstrap_peers = Some(peers);
        self
    }

    /// Public/private keypair for DHT node.
    pub fn key_material(mut self, key_material: &'a Ed25519KeyMaterial) -> Self {
        self.key_material = Some(key_material);
        self
    }

    /// Port to listen for incoming TCP connections.
    pub fn listening_port(mut self, port: u16) -> Self {
        let mut address = Multiaddr::empty();
        address.push(libp2p::multiaddr::Protocol::Ip4(Ipv4Addr::new(
            127, 0, 0, 1,
        )));
        address.push(libp2p::multiaddr::Protocol::Tcp(port));
        self.dht_config.listening_address = Some(address);
        self
    }

    /// How frequently, in seconds, the DHT attempts to
    /// dial peers found in its kbucket. Outside of tests,
    /// should not be lower than 5 seconds.
    pub fn peer_dialing_interval(mut self, interval: u64) -> Self {
        self.dht_config.peer_dialing_interval = interval;
        self
    }

    /// How long, in seconds, until a network query times out.
    pub fn query_timeout(mut self, timeout: u32) -> Self {
        self.dht_config.query_timeout = timeout;
        self
    }

    /// Default Time To Live (TTL) for records propagated to the network.
    pub fn ttl(mut self, ttl: u64) -> Self {
        self.ttl = ttl;
        self
    }

    /// Build a [NameSystem] based off of the provided configuration.
    pub fn build(mut self) -> Result<NameSystem<'a>> {
        let key_material = self
            .key_material
            .take()
            .ok_or_else(|| anyhow!("key_material required."))?;
        Ok(NameSystem {
            bootstrap_peers: self.bootstrap_peers.take(),
            dht: None,
            dht_config: self.dht_config,
            key_material,
            ttl: self.ttl,
            hosted_records: HashMap::new(),
            resolved_records: HashMap::new(),
        })
    }
}

impl<'a> Default for NameSystemBuilder<'a> {
    fn default() -> Self {
        Self {
            bootstrap_peers: None,
            ttl: 60 * 60 * 24, // 1 day
            dht_config: DHTConfig::default(),
            key_material: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use noosphere_core::authority::generate_ed25519_key;

    #[test]
    fn test_name_system_builder() -> Result<(), anyhow::Error> {
        let key_material = generate_ed25519_key();
        let bootstrap_peers: Vec<Multiaddr> = vec![
            "/ip4/127.0.0.50/tcp/33333/p2p/12D3KooWH8WgH9mgbMXrKX4veokUznvEn6Ycwg4qaGNi83nLkoUK"
                .parse()?,
            "/ip4/127.0.0.50/tcp/33334/p2p/12D3KooWMWo6tNGRx1G4TNqvr4SnHyVXSReC3tdX6zoJothXxV2c"
                .parse()?,
        ];

        let ns = NameSystemBuilder::default()
            .listening_port(30000)
            .key_material(&key_material)
            .bootstrap_peers(&bootstrap_peers)
            .bootstrap_interval(33)
            .peer_dialing_interval(11)
            .query_timeout(22)
            .ttl(3600)
            .build()?;

        assert_eq!(ns.key_material.0.as_ref(), key_material.0.as_ref());
        assert_eq!(ns.ttl, 3600);
        assert_eq!(ns.bootstrap_peers.as_ref().unwrap().len(), 2);
        assert_eq!(ns.bootstrap_peers.as_ref().unwrap()[0], bootstrap_peers[0],);
        assert_eq!(ns.bootstrap_peers.as_ref().unwrap()[1], bootstrap_peers[1]);
        assert_eq!(
            ns.dht_config.listening_address.as_ref().unwrap(),
            &"/ip4/127.0.0.1/tcp/30000".parse()?
        );
        assert_eq!(ns.dht_config.bootstrap_interval, 33);
        assert_eq!(ns.dht_config.peer_dialing_interval, 11);
        assert_eq!(ns.dht_config.query_timeout, 22);

        if let Ok(_) = NameSystemBuilder::default().build() {
            panic!("key_material required.");
        }
        Ok(())
    }
}
