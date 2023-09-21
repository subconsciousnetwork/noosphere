use crate::{
    dht::{DhtConfig, DhtError, DhtNode, DhtRecord, NetworkInfo, Peer},
    utils::make_p2p_address,
    validator::RecordValidator,
    DhtClient, PeerId,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use libp2p::{identity::Keypair, Multiaddr};
use noosphere_core::data::{Did, LinkRecord};
use ucan::{crypto::KeyMaterial, store::UcanJwtStore};
use ucan_key_support::ed25519::Ed25519KeyMaterial;

#[cfg(doc)]
use cid::Cid;

pub static BOOTSTRAP_PEERS_ADDRESSES: [&str; 1] =
    ["/ip4/134.122.20.28/tcp/6666/p2p/12D3KooWPyjAB3XWUboGmLLPkR53fTyj4GaNi65RvQ61BVwqV4HG"];

lazy_static! {
    /// Noosphere Name System's maintained list of peers to
    /// bootstrap nodes joining the network.
    pub static ref BOOTSTRAP_PEERS: [Multiaddr; 1] = BOOTSTRAP_PEERS_ADDRESSES.map(|addr| addr.parse().expect("parseable"));
}

pub trait NameSystemKeyMaterial: KeyMaterial + Clone {
    fn to_dht_keypair(&self) -> anyhow::Result<Keypair>;
}

impl NameSystemKeyMaterial for Ed25519KeyMaterial {
    fn to_dht_keypair(&self) -> anyhow::Result<Keypair> {
        pub const ED25519_KEY_LENGTH: usize = 32;
        let mut bytes: [u8; ED25519_KEY_LENGTH] = [0u8; ED25519_KEY_LENGTH];
        bytes[..ED25519_KEY_LENGTH].copy_from_slice(
            self.1
                .ok_or_else(|| anyhow!("Private key required in order to deserialize."))?
                .as_ref(),
        );
        let kp = Keypair::ed25519_from_bytes(&mut bytes)
            .map_err(|_| anyhow::anyhow!("Could not decode ED25519 key."))?;
        Ok(kp)
    }
}

/// The [NameSystem] is responsible for both propagating and resolving Sphere
/// DIDs into an authorized UCAN publish token, resolving into a
/// [Link<MemoIpld>] address for a sphere's content. These records are
/// propagated and resolved via the Noosphere Name System, a distributed
/// network, built on [libp2p](https://libp2p.io)'s [Kademlia DHT
/// specification](https://github.com/libp2p/specs/blob/master/kad-dht/README.md).
///
/// Hosted records can be set via [NameSystem::put_record], propagating the
/// record immediately, and repropagating on a specified interval. Records can
/// be resolved via [NameSystem::get_record].
///
/// See
/// <https://github.com/subconsciousnetwork/noosphere/blob/main/design/name-system.md>
/// for the full Noosphere Name System spec.
pub struct NameSystem {
    pub(crate) dht: DhtNode,
}

impl NameSystem {
    pub fn new<K: NameSystemKeyMaterial, S: UcanJwtStore + 'static>(
        key_material: &K,
        dht_config: DhtConfig,
        store: Option<S>,
    ) -> Result<Self> {
        let keypair = key_material.to_dht_keypair()?;
        let validator = store.map(|s| RecordValidator::new(s));

        Ok(NameSystem {
            dht: DhtNode::new(keypair, dht_config, validator)?,
        })
    }
}

#[async_trait]
impl DhtClient for NameSystem {
    /// Returns current network information for this node.
    async fn network_info(&self) -> Result<NetworkInfo> {
        self.dht.network_info().await.map_err(|e| e.into())
    }

    /// Returns current network information for this node.
    fn peer_id(&self) -> &PeerId {
        self.dht.peer_id()
    }

    /// Adds peers to connect to. Unless bootstrapping a network, at least one
    /// peer is needed.
    async fn add_peers(&self, peers: Vec<Multiaddr>) -> Result<()> {
        self.dht.add_peers(peers).await.map_err(|e| e.into())
    }

    /// Returns current network information for this node.
    async fn peers(&self) -> Result<Vec<Peer>> {
        self.dht.peers().await.map_err(|e| e.into())
    }

    /// Starts listening for connections on provided address.
    async fn listen(&self, listening_address: Multiaddr) -> Result<Multiaddr> {
        self.dht
            .listen(listening_address)
            .await
            .map_err(|e| e.into())
    }

    /// Stops listening for connections on provided address.
    async fn stop_listening(&self) -> Result<()> {
        self.dht.stop_listening().await.map_err(|e| e.into())
    }

    /// Connects to peers provided in `add_peers`.
    async fn bootstrap(&self) -> Result<()> {
        self.dht.bootstrap().await.map_err(|e| e.into())
    }

    /// Returns the listening addresses of this node.
    async fn address(&self) -> Result<Option<Multiaddr>> {
        let mut addresses = self
            .dht
            .addresses()
            .await
            .map_err(<DhtError as Into<anyhow::Error>>::into)?;
        if !addresses.is_empty() {
            let peer_id = self.peer_id().to_owned();
            let address = make_p2p_address(addresses.swap_remove(0), peer_id);
            Ok(Some(address))
        } else {
            Ok(None)
        }
    }

    async fn put_record(&self, record: LinkRecord, quorum: usize) -> Result<()> {
        let identity = record.to_sphere_identity();
        let record_bytes: Vec<u8> = record.try_into()?;
        match self
            .dht
            .put_record(identity.as_bytes(), &record_bytes, quorum)
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!(e.to_string())),
        }
    }

    async fn get_record(&self, identity: &Did) -> Result<Option<LinkRecord>> {
        match self.dht.get_record(identity.as_bytes()).await {
            Ok(DhtRecord { key: _, value }) => match value {
                Some(value) => Ok(Some(LinkRecord::try_from(value)?)),
                None => Ok(None),
            },
            Err(e) => Err(anyhow!(e.to_string())),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use noosphere_core::authority::generate_ed25519_key;

    #[test]
    fn bootstrap_peers_parseable() {
        // Force getting the lazy static ensuring the addresses
        // are valid Multiaddrs.
        assert_eq!(BOOTSTRAP_PEERS.len(), 1);
    }

    use crate::name_resolver_tests;
    async fn before_name_resolver_tests() -> Result<NameSystem> {
        let ns = {
            let key_material = generate_ed25519_key();
            let store = SphereDb::new(MemoryStorage::default()).await.unwrap();
            let ns = NameSystemBuilder::default()
                .ucan_store(store)
                .key_material(&key_material)
                .listening_port(0)
                .use_test_config()
                .build()
                .await
                .unwrap();
            ns.bootstrap().await.unwrap();
            ns
        };
        Ok(ns)
    }
    name_resolver_tests!(NameSystem, before_name_resolver_tests);

    use crate::dht_client_tests;
    use crate::{utils::wait_for_peers, NameSystemBuilder};
    use noosphere_storage::{MemoryStorage, SphereDb};
    use std::sync::Arc;
    use tokio::sync::Mutex;

    /// This struct is used to persist non-Client objects, like
    /// the name system and/or server, through the duration
    /// of each test.
    struct DataPlaceholder {
        _bootstrap: NameSystem,
        _ns: Arc<Mutex<NameSystem>>,
    }

    async fn before_each() -> Result<(DataPlaceholder, Arc<Mutex<NameSystem>>)> {
        let (bootstrap, bootstrap_address) = {
            let key_material = generate_ed25519_key();
            let store = SphereDb::new(MemoryStorage::default()).await.unwrap();
            let ns = NameSystemBuilder::default()
                .ucan_store(store)
                .key_material(&key_material)
                .listening_port(0)
                .use_test_config()
                .build()
                .await
                .unwrap();
            ns.bootstrap().await.unwrap();
            let address = ns.address().await?.unwrap();
            (ns, address)
        };

        let ns = {
            let key_material = generate_ed25519_key();
            let store = SphereDb::new(MemoryStorage::default()).await.unwrap();
            let ns = NameSystemBuilder::default()
                .ucan_store(store)
                .key_material(&key_material)
                .bootstrap_peers(&[bootstrap_address.clone()])
                .use_test_config()
                .build()
                .await
                .unwrap();
            ns.bootstrap().await.unwrap();
            wait_for_peers::<NameSystem>(&ns, 1).await?;
            ns
        };

        let client = Arc::new(Mutex::new(ns));
        // To align with implementations with discrete server/client
        // objects, we clone the NameSystem itself as the persistent
        // reference.
        let reference = client.clone();
        let data = DataPlaceholder {
            _ns: reference,
            _bootstrap: bootstrap,
        };
        Ok((data, client))
    }

    dht_client_tests!(NameSystem, before_each, DataPlaceholder);
}
