use crate::{
    dht::{DHTConfig, DHTNode},
    records::NSRecord,
};
use anyhow::{anyhow, Result};
use futures::future::try_join_all;
use libp2p::Multiaddr;
use noosphere_core::authority::SUPPORTED_KEYS;
use noosphere_storage::{db::SphereDb, interface::Store};
use std::collections::HashMap;
use ucan::crypto::did::DidParser;
use ucan_key_support::ed25519::Ed25519KeyMaterial;

/// The [NameSystem] is responsible for both propagating and resolving Sphere DIDs
/// into an authorized UCAN publish token, resolving into a [cid::Cid] address for
/// a sphere's content. These records are propagated and resolved via the
/// Noosphere NS distributed network, built on [libp2p](https://libp2p.io)'s
/// [Kademlia DHT specification](https://github.com/libp2p/specs/blob/master/kad-dht/README.md).
///
/// Hosted records can be set via [NameSystem::set_record], propagating the
/// record immediately, and repropagated every `propagation_interval` seconds. Records
/// can be resolved via [NameSystem::get_record].
///
/// New [NameSystem] instances can be created via [crate::NameSystemBuilder].
pub struct NameSystem<S>
where
    S: Store,
{
    /// Bootstrap peers for the DHT network.
    pub(crate) bootstrap_peers: Option<Vec<Multiaddr>>,
    pub(crate) dht: Option<DHTNode>,
    pub(crate) dht_config: DHTConfig,
    /// Key of the NameSystem's sphere.
    pub(crate) key_material: Ed25519KeyMaterial,
    pub(crate) store: SphereDb<S>,
    /// Map of sphere DIDs to [NSRecord] hosted/propagated by this name system.
    hosted_records: HashMap<String, NSRecord>,
    /// Map of resolved sphere DIDs to resolved [NSRecord].
    resolved_records: HashMap<String, NSRecord>,
    /// Cached DidParser.
    did_parser: DidParser,
}

impl<S> NameSystem<S>
where
    S: Store,
{
    /// Internal instantiation function invoked by [crate::NameSystemBuilder].
    pub(crate) fn new(
        key_material: Ed25519KeyMaterial,
        store: SphereDb<S>,
        bootstrap_peers: Option<Vec<Multiaddr>>,
        dht_config: DHTConfig,
    ) -> Self {
        NameSystem {
            key_material,
            store,
            bootstrap_peers,
            dht_config,
            dht: None,
            hosted_records: HashMap::new(),
            resolved_records: HashMap::new(),
            did_parser: DidParser::new(SUPPORTED_KEYS),
        }
    }

    /// Initializes and attempts to connect to the network.
    pub async fn connect(&mut self) -> Result<()> {
        let mut dht = DHTNode::new(
            &self.key_material,
            self.bootstrap_peers.as_ref(),
            &self.dht_config,
        )?;
        dht.run().map_err(|e| anyhow!(e.to_string()))?;
        dht.bootstrap().await.map_err(|e| anyhow!(e.to_string()))?;
        dht.wait_for_peers(1)
            .await
            .map_err(|e| anyhow!(e.to_string()))?;
        self.dht = Some(dht);
        Ok(())
    }

    /// Disconnect and deallocate connections to the network.
    pub fn disconnect(&mut self) -> Result<()> {
        if let Some(mut dht) = self.dht.take() {
            dht.terminate()?;
        }
        Ok(())
    }

    /// Propagates all hosted records on nearby peers in the DHT network.
    /// Automatically propagated by the intervals configured in [crate::NameSystemBuilder].
    ///
    /// Can fail if NameSystem is not connected or if no peers can be found.
    pub async fn propagate_records(&self) -> Result<()> {
        let _ = self.require_dht()?;

        if self.hosted_records.is_empty() {
            return Ok(());
        }

        let pending_tasks: Vec<_> = self
            .hosted_records
            .iter()
            .map(|(identity, record)| self.dht_set_record(identity, record))
            .collect();
        try_join_all(pending_tasks).await?;
        Ok(())
    }

    /// Propagates the corresponding managed sphere's [NSRecord] on nearby peers
    /// in the DHT network.
    ///
    /// Can fail if NameSystem is not connected or if no peers can be found.
    pub async fn set_record(&mut self, mut record: NSRecord) -> Result<()> {
        let _ = self.require_dht()?;

        record.validate(&self.store, &mut self.did_parser).await?;
        let identity = record.identity();

        self.dht_set_record(identity, &record).await?;
        self.hosted_records.insert(identity.to_owned(), record);
        Ok(())
    }

    /// Returns an [NSRecord] for the provided identity if found.
    ///
    /// Reads from local cache if a valid token is found; otherwise,
    /// queries the network for a valid record.
    ///
    /// Can fail if network errors occur.
    pub async fn get_record(&mut self, identity: &str) -> Result<Option<NSRecord>> {
        if let Some(record) = self.resolved_records.get(identity) {
            if !record.is_expired() {
                return Ok(Some(record.clone()));
            } else {
                self.resolved_records.remove(identity);
            }
        }
        // No non-expired record found locally, query the network.
        match self.dht_get_record(identity).await? {
            (_, Some(record)) => {
                self.resolved_records
                    .insert(identity.to_owned(), record.clone());
                Ok(Some(record))
            }
            (_, None) => Ok(None),
        }
    }

    /// Clears out the internal cache of resolved records.
    pub fn flush_records(&mut self) {
        self.resolved_records.drain();
    }

    /// Clears out the internal cache of resolved records
    /// for the matched identity. Returned value indicates whether
    /// a record was successfully removed.
    pub fn flush_records_for_identity(&mut self, identity: &String) -> bool {
        self.resolved_records.remove(identity).is_some()
    }

    /// Access the record cache of the name system.
    pub fn get_cache(&self) -> &HashMap<String, NSRecord> {
        &self.resolved_records
    }

    /// Access the record cache as mutable of the name system.
    pub fn get_cache_mut(&mut self) -> &mut HashMap<String, NSRecord> {
        &mut self.resolved_records
    }

    pub fn p2p_address(&self) -> Option<&Multiaddr> {
        if let Some(dht) = &self.dht {
            dht.p2p_address()
        } else {
            None
        }
    }

    /// Queries the DHT for a record for the given sphere identity.
    /// If no record is found, no error is returned.
    ///
    /// Returns an error if not connected to the DHT network.
    async fn dht_get_record(&self, identity: &str) -> Result<(String, Option<NSRecord>)> {
        let dht = self.require_dht()?;

        match dht.get_record(identity.to_owned().into_bytes()).await {
            Ok((_, result)) => match result {
                Some(value) => {
                    // Validation/correctness and filtering through
                    // the most recent values can be performed here
                    let record = NSRecord::try_from(value)?;
                    info!(
                        "NameSystem: GetRecord: {} {}",
                        identity,
                        record
                            .address()
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
    async fn dht_set_record(&self, identity: &str, record: &NSRecord) -> Result<()> {
        let dht = self.require_dht()?;

        match dht
            .set_record(String::from(identity).into_bytes(), record.try_into()?)
            .await
        {
            Ok(_) => {
                info!("NameSystem: SetRecord: {}", identity);
                Ok(())
            }
            Err(e) => {
                warn!("NameSystem: SetRecord: Failure for {} {:?}.", identity, e);
                Err(anyhow!(e.to_string()))
            }
        }
    }

    fn require_dht(&self) -> Result<&DHTNode> {
        self.dht.as_ref().ok_or_else(|| anyhow!("not connected"))
    }
}

impl<S> Drop for NameSystem<S>
where
    S: Store,
{
    fn drop(&mut self) {
        if let Err(e) = self.disconnect() {
            error!("{}", e.to_string());
        }
    }
}
