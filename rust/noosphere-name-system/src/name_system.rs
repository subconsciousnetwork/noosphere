use crate::dht::{DHTClient, DHTConfig};
use crate::utils;
use anyhow::Result;
/// @TODO these materials should be exposed in noosphere::authority?
use ucan_key_support::ed25519::Ed25519KeyMaterial;

pub struct NameSystem {
    dht: DHTClient,
}

impl NameSystem {
    /// Creates a new NameSystem for resolving Sphere DIDs into the most
    /// recent, verifiable CID for that sphere.
    /// Method can fail if [Ed25519KeyMaterial] cannot be decoded.
    pub fn new(key_material: &Ed25519KeyMaterial, query_timeout: u32) -> Result<Self> {
        let keypair = utils::key_material_to_libp2p_keypair(key_material)?;
        let dht = DHTClient::new(DHTConfig {
            keypair,
            query_timeout,
        });
        Ok(NameSystem { dht })
    }

    /// Get record associated with `key`.
    pub async fn get_record(&self, key: Vec<u8>) -> Result<Option<Vec<u8>>> {
        self.dht.get_record(key).await
    }

    /// Stores record `value` associated with `key`.
    pub async fn set_record(&self, key: Vec<u8>, value: Vec<u8>) -> Result<Vec<u8>> {
        self.dht.set_record(key, value).await
    }

    /// Initializes and attempts to connect to the network.
    pub async fn connect(&mut self) -> Result<()> {
        self.dht.connect().await
    }

    /// Disconnect and deallocate connections to the network.
    pub async fn disconnect(&mut self) -> Result<()> {
        self.dht.disconnect().await
    }
}
