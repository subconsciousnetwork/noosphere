use crate::{address_book::AddressBook, dht::DHTConfig, name_system::NameSystem};
use anyhow::{anyhow, Result};
use std::net::SocketAddr;
use ucan_key_support::ed25519::Ed25519KeyMaterial;

const DEFAULT_LISTENING_ADDR: &str = "127.0.0.1:0";
const DEFAULT_QUERY_TIMEOUT: u32 = 5 * 60;
const DEFAULT_BOOTSTRAP_INTERVAL: u64 = 5 * 60;
const DEFAULT_PEER_DIALING_INTERVAL: u64 = 5;

/// [NameSystemBuilder] is the primary external interface for
/// creating a new [NameSystem].
///
/// # Examples
///
/// ```
/// use noosphere::authority::generate_ed25519_key;
/// use noosphere_p2p::{NameSystem, NameSystemBuilder};
/// use anyhow::{Result, Error};
/// use std::net::SocketAddr;
///
/// fn main() -> Result<(), Error> {
///     let key_material = generate_ed25519_key();
///     let ns = NameSystemBuilder::default()
///         .key_material(&key_material)
///         .listening_address("127.0.0.1:30000".parse::<SocketAddr>()?)
///         .build()?;
///
///     Ok(())
/// }
/// ```
pub struct NameSystemBuilder<'a> {
    address_book: Option<AddressBook>,
    bootstrap_interval: u64,
    bootstrap_peers: Option<&'a Vec<String>>,
    key_material: Option<&'a Ed25519KeyMaterial>,
    // TODO(rust-lang/rust/#82485) Can take a reference here once
    // [SocketAddr::new] is const-stable.
    listening_address: Option<SocketAddr>,
    peer_dialing_interval: u64,
    query_timeout: u32,
}

impl<'a> NameSystemBuilder<'a> {
    /// AddressBook of values to propagate.
    pub fn address_book(mut self, address_book: AddressBook) -> Self {
        self.address_book = Some(address_book);
        self
    }

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
    pub fn bootstrap_peers(mut self, peers: &'a Vec<String>) -> Self {
        self.bootstrap_peers = Some(peers);
        self
    }

    /// Public/private keypair for DHT node.
    pub fn key_material(mut self, key_material: &'a Ed25519KeyMaterial) -> Self {
        self.key_material = Some(key_material);
        self
    }

    /// Address to listen for incoming connections.
    pub fn listening_address(mut self, address: SocketAddr) -> Self {
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

    /// Build a [NameSystem] based off of the provided configuration.
    pub fn build(mut self) -> Result<NameSystem<'a>> {
        let key_material = self
            .key_material
            .take()
            .ok_or_else(|| anyhow!("key_material required."))?;
        let listening_address = self
            .listening_address
            .take()
            .ok_or_else(|| anyhow!("listening_address required."))?;
        Ok(NameSystem {
            address_book: self.address_book.take(),
            bootstrap_peers: self.bootstrap_peers,
            dht: None,
            dht_config: DHTConfig {
                bootstrap_interval: self.bootstrap_interval,
                peer_dialing_interval: self.peer_dialing_interval,
                query_timeout: self.query_timeout,
            },
            key_material,
            listening_address,
        })
    }
}

impl<'a> Default for NameSystemBuilder<'a> {
    fn default() -> Self {
        Self {
            address_book: None,
            bootstrap_interval: DEFAULT_BOOTSTRAP_INTERVAL,
            bootstrap_peers: None,
            key_material: None,
            listening_address: Some(
                DEFAULT_LISTENING_ADDR
                    .parse::<SocketAddr>()
                    .expect("default address must be parseable."),
            ),
            peer_dialing_interval: DEFAULT_PEER_DIALING_INTERVAL,
            query_timeout: DEFAULT_QUERY_TIMEOUT,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use noosphere::authority::generate_ed25519_key;

    #[test]
    fn test_name_system_builder() -> Result<(), anyhow::Error> {
        let key_material = generate_ed25519_key();
        let bootstrap_peers = vec![
            String::from("/ip4/127.0.0.50/tcp/33333/p2p/12D3KooWH8WgH9mgbMXrKX4veokUznvEn6Ycwg4qaGNi83nLkoUK"),
            String::from("/ip4/127.0.0.50/tcp/33334/p2p/12D3KooWMWo6tNGRx1G4TNqvr4SnHyVXSReC3tdX6zoJothXxV2c"),
        ];
        let listening_address: SocketAddr = "127.0.0.1:12000".parse()?;

        let ns = NameSystemBuilder::default()
            .listening_address(listening_address)
            .key_material(&key_material)
            .bootstrap_peers(&bootstrap_peers)
            .bootstrap_interval(33)
            .peer_dialing_interval(11)
            .query_timeout(22)
            .build()?;

        assert_eq!(ns.key_material.0.as_ref(), key_material.0.as_ref());
        assert_eq!(ns.listening_address, listening_address);
        assert_eq!(ns.bootstrap_peers.unwrap().len(), 2);
        assert_eq!(ns.bootstrap_peers.unwrap()[0], bootstrap_peers[0],);
        assert_eq!(ns.bootstrap_peers.unwrap()[1], bootstrap_peers[1]);
        assert_eq!(ns.dht_config.bootstrap_interval, 33);
        assert_eq!(ns.dht_config.peer_dialing_interval, 11);
        assert_eq!(ns.dht_config.query_timeout, 22);
        if let Ok(_) = NameSystemBuilder::default().build() {
            panic!("key_material required.");
        }
        Ok(())
    }
}
