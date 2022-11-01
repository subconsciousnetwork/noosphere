use crate::{
    dht::{DHTConfig, DHTNode},
    records::NSRecord,
};
use anyhow::{anyhow, Result};
use cid::Cid;
use futures::future::try_join_all;
use libp2p::Multiaddr;
use std::collections::HashMap;
use ucan_key_support::ed25519::Ed25519KeyMaterial;

/// The [NameSystem] is responsible for both propagating and resolving Sphere DIDs
/// into [NSRecord]s, containing data to find an authenticated [Cid] link to the
/// sphere's content. These records are propagated and resolved via the
/// Noosphere NS distributed network, built on [libp2p](https://libp2p.io)'s
/// [Kademlia DHT specification](https://github.com/libp2p/specs/blob/master/kad-dht/README.md).
///
/// Hosted records can be set via [NameSystem::set_record], propagating the
/// record immediately, and repropagated every `ttl` seconds. Records
/// can be resolved via [NameSystem::get_record].
///
/// New [NameSystem] instances can be created via [crate::NameSystemBuilder].
pub struct NameSystem<'a> {
    /// Bootstrap peers for the DHT network.
    pub(crate) bootstrap_peers: Option<&'a Vec<Multiaddr>>,
    pub(crate) dht: Option<DHTNode>,
    pub(crate) dht_config: DHTConfig,
    /// Key of the NameSystem's sphere.
    pub(crate) key_material: &'a Ed25519KeyMaterial,
    /// In seconds, the Time-To-Live (TTL) duration of records set
    /// in the network, and implicitly, how often the records are
    /// propagated.
    pub(crate) ttl: u64,
    /// Map of sphere DIDs to [NSRecord] hosted/propagated by this name system.
    pub(crate) hosted_records: HashMap<String, NSRecord>,
    /// Map of resolved sphere DIDs to resolved [NSRecord].
    pub(crate) resolved_records: HashMap<String, NSRecord>,
}

impl<'a> NameSystem<'a> {
    /// Initializes and attempts to connect to the network.
    pub async fn connect(&mut self) -> Result<()> {
        let mut dht = DHTNode::new(self.key_material, self.bootstrap_peers, &self.dht_config)?;
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
    /// in the DHT network. Automatically called every `ttl` seconds (TBD),
    /// but can be manually called push updated records to the network.
    ///
    /// Can fail if NameSystem is not connected or if no peers can be found.
    pub async fn propagate_records(&self) -> Result<()> {
        if self.dht.is_none() {
            return Err(anyhow!("not connected"));
        }
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

    /// Propagates the corresponding managed sphere's content Cid on nearby peers
    /// in the DHT network.
    ///
    /// Can fail if NameSystem is not connected or if no peers can be found.
    pub async fn set_record(&mut self, identity: &String, link: &Cid) -> Result<()> {
        if self.dht.is_none() {
            return Err(anyhow!("not connected"));
        }

        let record = NSRecord::new_from_ttl(link.to_owned(), self.ttl)?;
        self.dht_set_record(identity, &record).await?;
        self.hosted_records.insert(identity.to_owned(), record);
        Ok(())
    }

    /// Gets the content Cid for the provided sphere identity.
    pub async fn get_record(&mut self, identity: &String) -> Option<&Cid> {
        // Round about way of checking for local valid record before
        // querying the DHT network due to the borrow checker.
        // https://stackoverflow.com/questions/70779967/rust-borrow-checker-and-early-returns#
        let has_valid_record: bool = {
            if let Some(record) = self.resolved_records.get(identity) {
                !record.is_expired()
            } else {
                false
            }
        };

        // No non-expired record found locally, query the network.
        if !has_valid_record {
            match self.dht_get_record(identity).await {
                Ok((_, Some(record))) => {
                    self.resolved_records.insert(identity.to_owned(), record);
                }
                Ok((_, None)) => {}
                Err(_) => {}
            }
        }
        match self.resolved_records.get(identity) {
            Some(record) => Some(&record.cid),
            None => None,
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
    async fn dht_get_record(&self, identity: &String) -> Result<(String, Option<NSRecord>)> {
        let dht = self.dht.as_ref().ok_or_else(|| anyhow!("not connected"))?;
        match dht.get_record(identity.to_owned().into_bytes()).await {
            Ok((_, result)) => match result {
                Some(value) => {
                    // Validation/correctness and filtering through
                    // the most recent values can be performed here
                    let record = NSRecord::from_bytes(value)?;
                    info!("NameSystem: GetRecord: {} {}", identity, &record.cid);
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
    /// Can fail if NameSystem is not connected or if no peers can be found.
    async fn dht_set_record(&self, identity: &String, record: &NSRecord) -> Result<()> {
        let dht = self.dht.as_ref().ok_or_else(|| anyhow!("not connected"))?;

        match dht
            .set_record(identity.to_owned().into_bytes(), record.to_bytes()?)
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
}

impl<'a> Drop for NameSystem<'a> {
    fn drop(&mut self) {
        if let Err(e) = self.disconnect() {
            error!("{}", e.to_string());
        }
    }
}
