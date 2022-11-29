use crate::{
    dht::{DHTConfig, DHTKeyMaterial},
    name_system::NameSystem,
};
use anyhow::{anyhow, Result};
use libp2p::{self, Multiaddr};
use noosphere_storage::{SphereDb, Storage};
use std::net::Ipv4Addr;

#[cfg(doc)]
use libp2p::kad::KademliaConfig;

/// [NameSystemBuilder] is the primary external interface for
/// creating a new [NameSystem]. `key_material` and `store`
/// must be provided.
///
/// # Examples
///
/// ```
/// use noosphere_core::authority::generate_ed25519_key;
/// use noosphere_storage::{SphereDb, MemoryStorage};
/// use noosphere_ns::{NameSystem, NameSystemBuilder};
/// use ucan_key_support::ed25519::Ed25519KeyMaterial;
/// use tokio;
///
/// #[tokio::main]
/// async fn main() {
///     let key_material = generate_ed25519_key();
///     let store = SphereDb::new(&MemoryStorage::default()).await.unwrap();
///
///     let ns = NameSystemBuilder::default()
///         .key_material(&key_material)
///         .store(&store)
///         .listening_port(30000)
///         .build().expect("valid config");
///
///     assert!(NameSystemBuilder::<MemoryStorage, Ed25519KeyMaterial>::default().build().is_err(),
///         "key_material and store must be provided.");
/// }
/// ```
pub struct NameSystemBuilder<S, K>
where
    S: Storage,
    K: DHTKeyMaterial,
{
    bootstrap_peers: Option<Vec<Multiaddr>>,
    dht_config: DHTConfig,
    key_material: Option<K>,
    store: Option<SphereDb<S>>,
}

impl<S, K> NameSystemBuilder<S, K>
where
    S: Storage,
    K: DHTKeyMaterial,
{
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
    pub fn bootstrap_peers(mut self, peers: &[Multiaddr]) -> Self {
        self.bootstrap_peers = Some(peers.to_owned());
        self
    }

    /// Public/private keypair for DHT node.
    pub fn key_material(mut self, key_material: &K) -> Self {
        self.key_material = Some(key_material.to_owned());
        self
    }

    /// Port to listen for incoming TCP connections. If not specified,
    /// an open port is automatically chosen.
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

    /// How long, in seconds, published records are replicated to
    /// peers. Should be significantly shorter than `record_ttl`.
    /// See [KademliaConfig::set_publication_interval] and [KademliaConfig::set_provider_publication_interval].
    pub fn publication_interval(mut self, interval: u32) -> Self {
        self.dht_config.publication_interval = interval;
        self
    }

    /// How long, in seconds, until a network query times out.
    pub fn query_timeout(mut self, timeout: u32) -> Self {
        self.dht_config.query_timeout = timeout;
        self
    }

    /// How long, in seconds, records remain valid for. Should be significantly
    /// longer than `publication_interval`.
    /// See [KademliaConfig::set_record_ttl] and [KademliaConfig::set_provider_record_ttl].
    pub fn record_ttl(mut self, interval: u32) -> Self {
        self.dht_config.record_ttl = interval;
        self
    }

    /// How long, in seconds, stored records are replicated to
    /// peers. Should be significantly shorter than `publication_interval`.
    /// See [KademliaConfig::set_replication_interval].
    pub fn replication_interval(mut self, interval: u32) -> Self {
        self.dht_config.replication_interval = interval;
        self
    }

    /// The Noosphere Store to use for reading and writing sphere data.
    pub fn store(mut self, store: &SphereDb<S>) -> Self {
        self.store = Some(store.to_owned());
        self
    }

    /// Build a [NameSystem] based off of the provided configuration.
    pub fn build(mut self) -> Result<NameSystem<S, K>> {
        let key_material = self
            .key_material
            .take()
            .ok_or_else(|| anyhow!("key_material required."))?;
        let store = self
            .store
            .take()
            .ok_or_else(|| anyhow!("store required."))?;
        Ok(NameSystem::new(
            key_material,
            store,
            self.bootstrap_peers.take(),
            self.dht_config,
        ))
    }
}

impl<S, K> Default for NameSystemBuilder<S, K>
where
    S: Storage,
    K: DHTKeyMaterial,
{
    fn default() -> Self {
        Self {
            bootstrap_peers: None,
            dht_config: DHTConfig::default(),
            key_material: None,
            store: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use noosphere_core::authority::generate_ed25519_key;
    use noosphere_storage::{MemoryStorage, SphereDb};
    use ucan_key_support::ed25519::Ed25519KeyMaterial;

    #[tokio::test]
    async fn test_name_system_builder() -> Result<(), anyhow::Error> {
        let key_material = generate_ed25519_key();
        let store = SphereDb::new(&MemoryStorage::default()).await.unwrap();
        let bootstrap_peers: Vec<Multiaddr> = vec![
            "/ip4/127.0.0.50/tcp/33333/p2p/12D3KooWH8WgH9mgbMXrKX4veokUznvEn6Ycwg4qaGNi83nLkoUK"
                .parse()?,
            "/ip4/127.0.0.50/tcp/33334/p2p/12D3KooWMWo6tNGRx1G4TNqvr4SnHyVXSReC3tdX6zoJothXxV2c"
                .parse()?,
        ];

        let ns = NameSystemBuilder::default()
            .listening_port(30000)
            .key_material(&key_material)
            .store(&store)
            .bootstrap_peers(&bootstrap_peers)
            .bootstrap_interval(33)
            .peer_dialing_interval(11)
            .query_timeout(22)
            .publication_interval(60 * 60 * 24 + 1)
            .replication_interval(60 * 60 + 1)
            .record_ttl(60 * 60 * 24 * 3 + 1)
            .build()?;

        assert_eq!(ns.key_material.0.as_ref(), key_material.0.as_ref());
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
        assert_eq!(ns.dht_config.publication_interval, 60 * 60 * 24 + 1);
        assert_eq!(ns.dht_config.replication_interval, 60 * 60 + 1);
        assert_eq!(ns.dht_config.record_ttl, 60 * 60 * 24 * 3 + 1);

        if NameSystemBuilder::<MemoryStorage, Ed25519KeyMaterial>::default()
            .store(&store)
            .build()
            .is_ok()
        {
            panic!("key_material required.");
        }
        if NameSystemBuilder::<MemoryStorage, Ed25519KeyMaterial>::default()
            .key_material(&key_material)
            .build()
            .is_ok()
        {
            panic!("store required.");
        }
        Ok(())
    }
}
