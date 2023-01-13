use crate::{
    client::NameSystemClient,
    dht::{DHTConfig, DHTError, DHTKeyMaterial, DHTNode, DHTRecord, NetworkInfo, Peer},
    records::NSRecord,
    utils::make_p2p_address,
    validator::Validator,
    PeerId,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
#[cfg(doc)]
use cid::Cid;
use futures::future::try_join_all;
use libp2p::Multiaddr;
use noosphere_core::data::Did;
use noosphere_storage::{SphereDb, Storage};
use std::collections::HashMap;
use tokio::sync::{Mutex, MutexGuard};

pub static BOOTSTRAP_PEERS_ADDRESSES: [&str; 1] =
    ["/ip4/134.122.20.28/tcp/6666/p2p/12D3KooWAKxaCWsSGauqhCZXyeYjDSQ6jna2SmVhLn4J7uQFdvot"];

lazy_static! {
    /// Noosphere Name System's maintained list of peers to
    /// bootstrap nodes joining the network.
    pub static ref BOOTSTRAP_PEERS: [Multiaddr; 1] = BOOTSTRAP_PEERS_ADDRESSES.map(|addr| addr.parse().expect("parseable"));
}

/// The [NameSystem] is responsible for both propagating and resolving Sphere DIDs
/// into an authorized UCAN publish token, resolving into a [Cid] address for
/// a sphere's content. These records are propagated and resolved via the
/// Noosphere Name System, a distributed network, built on [libp2p](https://libp2p.io)'s
/// [Kademlia DHT specification](https://github.com/libp2p/specs/blob/master/kad-dht/README.md).
///
/// Hosted records can be set via [NameSystem::put_record], propagating the
/// record immediately, and repropagating on a specified interval. Records
/// can be resolved via [NameSystem::get_record].
///
/// See <https://github.com/subconsciousnetwork/noosphere/blob/main/design/name-system.md> for
/// the full Noosphere Name System spec.
pub struct NameSystem {
    pub(crate) dht: DHTNode,
    /// Map of sphere DIDs to [NSRecord] hosted/propagated by this name system.
    hosted_records: Mutex<HashMap<Did, NSRecord>>,
    /// Map of resolved sphere DIDs to resolved [NSRecord].
    resolved_records: Mutex<HashMap<Did, NSRecord>>,

    #[cfg(feature = "api_server")]
    api_server: Option<APIServer>,
}

impl NameSystem {
    pub fn new<S: Storage + 'static, K: DHTKeyMaterial>(
        key_material: &K,
        store: SphereDb<S>,
        dht_config: DHTConfig,
    ) -> Result<Self> {
        Ok(NameSystem {
            dht: DHTNode::new(key_material, dht_config, Some(Validator::new(store)))?,
            hosted_records: Mutex::new(HashMap::new()),
            resolved_records: Mutex::new(HashMap::new()),
        })
    }

    /// Propagates all hosted records on nearby peers in the DHT network.
    /// Automatically propagated by the intervals configured in provided [DHTConfig].
    ///
    /// Can fail if NameSystem is not connected or if no peers can be found.
    pub async fn propagate_records(&self) -> Result<()> {
        let hosted_records = self.hosted_records.lock().await;

        if hosted_records.is_empty() {
            return Ok(());
        }

        let pending_tasks: Vec<_> = hosted_records
            .iter()
            .map(|(identity, record)| self.dht_put_record(identity, record))
            .collect();
        try_join_all(pending_tasks).await?;
        Ok(())
    }

    /// Clears out the internal cache of resolved records.
    pub async fn flush_records(&self) {
        let mut resolved_records = self.resolved_records.lock().await;
        resolved_records.drain();
    }

    /// Clears out the internal cache of resolved records
    /// for the matched identity. Returned value indicates whether
    /// a record was successfully removed.
    pub async fn flush_records_for_identity(&self, identity: &Did) -> bool {
        let mut resolved_records = self.resolved_records.lock().await;
        resolved_records.remove(identity).is_some()
    }

    /// Access the record cache of the name system.
    pub async fn get_cache(&self) -> MutexGuard<HashMap<Did, NSRecord>> {
        self.resolved_records.lock().await
    }

    /// Queries the DHT for a record for the given sphere identity.
    /// If no record is found, no error is returned.
    ///
    /// Returns an error if not connected to the DHT network.
    async fn dht_get_record(&self, identity: &Did) -> Result<(Did, Option<NSRecord>)> {
        match self.dht.get_record(identity.as_bytes()).await {
            Ok(DHTRecord { key: _, value }) => match value {
                Some(value) => {
                    // Validation/correctness and filtering through
                    // the most recent values can be performed here
                    let record = NSRecord::try_from(value)?;
                    info!(
                        "NameSystem: GetRecord: {} {}",
                        identity,
                        record
                            .link()
                            .map_or_else(|| String::from("None"), |cid| cid.to_string())
                    );
                    Ok((identity.to_owned(), Some(record)))
                }
                None => {
                    warn!("NameSystem: GetRecord: No record found for {}.", identity);
                    Ok((identity.to_owned(), None))
                }
            },
            Err(e) => {
                warn!("NameSystem: GetRecord: Failure for {} {:?}.", identity, e);
                Err(anyhow!(e.to_string()))
            }
        }
    }

    /// Propagates and serializes the record on peers in the DHT network.
    ///
    /// Can fail if record is invalid, NameSystem is not connected or
    /// if no peers can be found.
    async fn dht_put_record(&self, identity: &Did, record: &NSRecord) -> Result<()> {
        let record: Vec<u8> = record.try_into()?;
        match self.dht.put_record(identity.as_bytes(), &record).await {
            Ok(_) => {
                info!("NameSystem: PutRecord: {}", identity);
                Ok(())
            }
            Err(e) => {
                warn!("NameSystem: PutRecord: Failure for {} {:?}.", identity, e);
                Err(anyhow!(e.to_string()))
            }
        }
    }
}

#[async_trait]
impl NameSystemClient for NameSystem {
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
            .map_err(|e| <DHTError as Into<anyhow::Error>>::into(e))?;
        if addresses.len() >= 1 {
            let peer_id = self.peer_id().to_owned();
            let address = make_p2p_address(addresses.swap_remove(0), peer_id);
            Ok(Some(address))
        } else {
            Ok(None)
        }
    }

    /// Propagates the corresponding managed sphere's [NSRecord] on nearby peers
    /// in the DHT network.
    ///
    /// Can fail if NameSystem is not connected or if no peers can be found.
    async fn put_record(&self, record: NSRecord) -> Result<()> {
        let identity = Did::from(record.identity());
        self.dht_put_record(&identity, &record).await?;
        self.hosted_records.lock().await.insert(identity, record);
        Ok(())
    }

    /// Returns an [NSRecord] for the provided identity if found.
    ///
    /// Reads from local cache if a valid token is found; otherwise,
    /// queries the network for a valid record.
    ///
    /// Can fail if network errors occur.
    async fn get_record(&self, identity: &Did) -> Result<Option<NSRecord>> {
        {
            let mut resolved_records = self.resolved_records.lock().await;
            if let Some(record) = resolved_records.get(identity) {
                if !record.is_expired() {
                    return Ok(Some(record.clone()));
                } else {
                    resolved_records.remove(identity);
                }
            }
        };

        // No non-expired record found locally, query the network.
        match self.dht_get_record(identity).await? {
            (_, Some(record)) => {
                let mut resolved_records = self.resolved_records.lock().await;
                resolved_records.insert(identity.to_owned(), record.clone());
                Ok(Some(record))
            }
            (_, None) => Ok(None),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn bootstrap_peers_parseable() {
        // Force getting the lazy static ensuring the addresses
        // are valid Multiaddrs.
        assert_eq!(BOOTSTRAP_PEERS.len(), 1);
    }

    use crate::ns_client_tests;
    use crate::{utils::wait_for_peers, NameSystemBuilder, NameSystemClient};
    use noosphere_core::authority::generate_ed25519_key;
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
            let store = SphereDb::new(&MemoryStorage::default()).await.unwrap();
            let ns = NameSystemBuilder::default()
                .key_material(&key_material)
                .store(&store)
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
            let store = SphereDb::new(&MemoryStorage::default()).await.unwrap();
            let ns = NameSystemBuilder::default()
                .key_material(&key_material)
                .store(&store)
                .bootstrap_peers(&vec![bootstrap_address.clone()])
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

    ns_client_tests!(NameSystem, before_each, DataPlaceholder);
}
