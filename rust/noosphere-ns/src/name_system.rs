use crate::{
    dht::{DHTConfig, DHTKeyMaterial, DHTNode, DHTRecord},
    records::NSRecord,
    validator::Validator,
};
use anyhow::{anyhow, Result};
use futures::future::try_join_all;
use libp2p::Multiaddr;
use noosphere_core::data::Did;
use noosphere_storage::{SphereDb, Storage};
use std::collections::HashMap;
use tokio::sync::{Mutex, MutexGuard};

#[cfg(doc)]
use cid::Cid;

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

    /// Adds peers to connect to. Unless bootstrapping a network, at least one
    /// peer is needed.
    pub async fn add_peers(&self, peers: Vec<Multiaddr>) -> Result<()> {
        self.dht.add_peers(peers).await.map_err(|e| e.into())
    }

    /// Starts listening for connections on provided address.
    pub async fn start_listening(&self, listening_address: Multiaddr) -> Result<()> {
        self.dht
            .start_listening(listening_address)
            .await
            .map_err(|e| e.into())
    }

    /// Stops listening for connections on provided address.
    pub async fn stop_listening(&self, listening_address: Multiaddr) -> Result<()> {
        self.dht
            .stop_listening(listening_address)
            .await
            .map_err(|e| e.into())
    }

    /// Connects to peers provided in `add_peers`.
    pub async fn bootstrap(&self) -> Result<()> {
        self.dht.bootstrap().await.map_err(|e| e.into())
    }

    /// Returns the listening addresses of this node.
    pub async fn addresses(&self) -> Result<Vec<Multiaddr>> {
        self.dht.addresses().await.map_err(|e| e.into())
    }

    /// Returns the listening addresses of this node as a P2P address.
    pub async fn p2p_addresses(&self) -> Result<Vec<Multiaddr>> {
        self.dht.p2p_addresses().await.map_err(|e| e.into())
    }

    /// Asynchronously wait until this name system node is connected
    /// to at least `requested_peers` number of peers.
    pub async fn wait_for_peers(&self, requested_peers: usize) -> Result<()> {
        self.dht
            .wait_for_peers(requested_peers)
            .await
            .map_err(|e| e.into())
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

    /// Propagates the corresponding managed sphere's [NSRecord] on nearby peers
    /// in the DHT network.
    ///
    /// Can fail if NameSystem is not connected or if no peers can be found.
    pub async fn put_record(&self, record: NSRecord) -> Result<()> {
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
    pub async fn get_record(&self, identity: &Did) -> Result<Option<NSRecord>> {
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn bootstrap_peers_parseable() {
        // Force getting the lazy static ensuring the addresses
        // are valid Multiaddrs.
        assert_eq!(BOOTSTRAP_PEERS.len(), 1);
    }
}
