use crate::{dht::DhtConfig, name_system::NameSystem, DhtClient, NameSystemKeyMaterial};
use anyhow::{anyhow, Result};
use libp2p::{self, Multiaddr};
use std::net::Ipv4Addr;
use ucan::store::UcanJwtStore;

#[cfg(doc)]
use libp2p::kad::KademliaConfig;

/// [NameSystemBuilder] is an alternate interface for
/// creating a new [NameSystem]. `key_material` and `store`
/// must be provided.
///
/// # Examples
///
/// ```
/// use noosphere_core::authority::generate_ed25519_key;
/// use noosphere_storage::{SphereDb, MemoryStorage};
/// use noosphere_ns::{BOOTSTRAP_PEERS, NameSystem, DhtClient, NameSystemBuilder};
/// use ucan_key_support::ed25519::Ed25519KeyMaterial;
/// use tokio;
///
/// #[tokio::main(flavor = "multi_thread")]
/// async fn main() {
///     let key_material = generate_ed25519_key();
///     let store = SphereDb::new(MemoryStorage::default()).await.unwrap();
///
///     let ns = NameSystemBuilder::default()
///         .ucan_store(store)
///         .key_material(&key_material)
///         .listening_port(30000)
///         .bootstrap_peers(&BOOTSTRAP_PEERS[..])
///         .build().await.unwrap();
///     ns.bootstrap().await.unwrap();
/// }
/// ```
pub struct NameSystemBuilder<K, S>
where
    K: NameSystemKeyMaterial + 'static,
    S: UcanJwtStore + 'static,
{
    bootstrap_peers: Option<Vec<Multiaddr>>,
    listening_address: Option<Multiaddr>,
    dht_config: DhtConfig,
    key_material: Option<K>,
    ucan_store: Option<S>,
}

impl<K, S> NameSystemBuilder<K, S>
where
    K: NameSystemKeyMaterial + 'static,
    S: UcanJwtStore + 'static,
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
        self.listening_address = Some(address);
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

    pub fn ucan_store(mut self, store: S) -> Self {
        self.ucan_store = Some(store);
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

    /// Build a [NameSystem] based off of the provided configuration.
    pub async fn build(mut self) -> Result<NameSystem> {
        let key_material = self
            .key_material
            .take()
            .ok_or_else(|| anyhow!("key_material required."))?;
        let ucan_store = self
            .ucan_store
            .ok_or_else(|| anyhow!("ucan_store is required"))?;
        let ns = NameSystem::new(&key_material, self.dht_config.clone(), Some(ucan_store))?;

        if let Some(listening_address) = self.listening_address {
            ns.listen(listening_address).await?;
        }

        if let Some(bootstrap_peers) = self.bootstrap_peers {
            ns.add_peers(bootstrap_peers).await?;
        }

        Ok(ns)
    }

    #[cfg(test)]
    /// Helper method to configure a NameSystem instance to
    /// use test-friendly values when running in CI.
    pub fn use_test_config(mut self) -> Self {
        self.dht_config.peer_dialing_interval = 1;
        self
    }
}

impl<K, S> Default for NameSystemBuilder<K, S>
where
    K: NameSystemKeyMaterial + 'static,
    S: UcanJwtStore + 'static,
{
    fn default() -> Self {
        Self {
            bootstrap_peers: None,
            dht_config: DhtConfig::default(),
            key_material: None,
            listening_address: None,
            ucan_store: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::PeerId;
    use noosphere_core::authority::generate_ed25519_key;
    use noosphere_storage::{MemoryStorage, SphereDb};
    use ucan_key_support::ed25519::Ed25519KeyMaterial;

    #[tokio::test]
    async fn test_name_system_builder() -> Result<(), anyhow::Error> {
        let key_material = generate_ed25519_key();
        let peer_id = {
            let keypair = key_material.to_dht_keypair()?;
            PeerId::from(keypair.public())
        };
        let store = SphereDb::new(MemoryStorage::default()).await.unwrap();
        let bootstrap_peers: Vec<Multiaddr> = vec![
            "/ip4/127.0.0.50/tcp/33333/p2p/12D3KooWH8WgH9mgbMXrKX4veokUznvEn6Ycwg4qaGNi83nLkoUK"
                .parse()?,
            "/ip4/127.0.0.50/tcp/33334/p2p/12D3KooWMWo6tNGRx1G4TNqvr4SnHyVXSReC3tdX6zoJothXxV2c"
                .parse()?,
        ];

        let ns = NameSystemBuilder::default()
            .listening_port(30000)
            .ucan_store(store.clone())
            .key_material(&key_material)
            .bootstrap_peers(&bootstrap_peers)
            .bootstrap_interval(33)
            .peer_dialing_interval(11)
            .query_timeout(22)
            .publication_interval(60 * 60 * 24 + 1)
            .replication_interval(60 * 60 + 1)
            .record_ttl(60 * 60 * 24 * 3 + 1)
            .build()
            .await?;

        assert_eq!(ns.dht.peer_id(), &peer_id);
        assert_eq!(
            ns.address().await?.unwrap(),
            format!("/ip4/127.0.0.1/tcp/30000/p2p/{}", peer_id).parse()?
        );
        let dht_config = ns.dht.config();
        assert_eq!(dht_config.bootstrap_interval, 33);
        assert_eq!(dht_config.peer_dialing_interval, 11);
        assert_eq!(dht_config.query_timeout, 22);
        assert_eq!(dht_config.publication_interval, 60 * 60 * 24 + 1);
        assert_eq!(dht_config.replication_interval, 60 * 60 + 1);
        assert_eq!(dht_config.record_ttl, 60 * 60 * 24 * 3 + 1);

        if NameSystemBuilder::<Ed25519KeyMaterial, _>::default()
            .ucan_store(store.clone())
            .build()
            .await
            .is_ok()
        {
            panic!("key_material required.");
        }
        if NameSystemBuilder::<Ed25519KeyMaterial, SphereDb<MemoryStorage>>::default()
            .key_material(&key_material)
            .build()
            .await
            .is_ok()
        {
            panic!("ucan_store required.");
        }
        Ok(())
    }
}
