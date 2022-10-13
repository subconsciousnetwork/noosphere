use crate::dht::{DHTClient, DHTConfig, DHTError};
/// @TODO these materials should be exposed in noosphere::authority?
use ucan_key_support::ed25519::Ed25519KeyMaterial;

pub struct NameSystemConfig<'a> {
    /// ed25519 key material containing public and private keys.
    pub key_material: Option<&'a Ed25519KeyMaterial>,
    /// Set query timeout in seconds.
    pub query_timeout: u32,
    /// Port to listen for incoming connections when running as a server.
    /// `Some(0)` will automatically choose an appropriate port.
    /// `None` will indicate client-only usage.
    pub server_port: Option<u16>,
    /// List of bootstrap nodes to connect to in [libp2p::Multiaddr] form.
    pub bootstrap_peers: Option<Vec<String>>,
}

impl<'a> Default for NameSystemConfig<'a> {
    fn default() -> Self {
        Self {
            key_material: None,
            query_timeout: 5 * 60,
            server_port: None,
            bootstrap_peers: None,
        }
    }
}

/// See [NameSystemConfig] for details on fields.
pub struct NameSystemBuilder<'a> {
    config: NameSystemConfig<'a>,
}

impl<'a> NameSystemBuilder<'a> {
    pub fn new() -> Self {
        NameSystemBuilder {
            config: NameSystemConfig::default(),
        }
    }

    pub fn key_material(mut self, key_material: &'a Ed25519KeyMaterial) -> Self {
        self.config.key_material = Some(key_material);
        self
    }

    pub fn query_timeout(mut self, query_timeout: u32) -> Self {
        self.config.query_timeout = query_timeout;
        self
    }

    pub fn server_port(mut self, server_port: u16) -> Self {
        self.config.server_port = Some(server_port);
        self
    }

    pub fn bootstrap_peers(mut self, bootstrap_peers: Vec<String>) -> Self {
        self.config.bootstrap_peers = Some(bootstrap_peers);
        self
    }

    pub fn build(mut self) -> Result<NameSystem, DHTError> {
        let ns = NameSystem::new(self.config)?;
        Ok(ns)
    }
}

pub struct NameSystem {
    dht: DHTClient,
}

impl NameSystem {
    /// Creates a new NameSystem for resolving Sphere DIDs into the most
    /// recent, verifiable CID for that sphere.
    /// Method can fail if [Ed25519KeyMaterial] cannot be decoded.
    pub fn new(config: NameSystemConfig) -> Result<Self, DHTError> {
        let dht = DHTClient::new(DHTConfig::try_from(config)?);
        Ok(NameSystem { dht })
    }

    pub async fn start_providing(&self, key: Vec<u8>) -> Result<(), DHTError> {
        self.dht.start_providing(key).await
    }

    /// Get record associated with `key`.
    pub async fn get_record(&self, key: Vec<u8>) -> Result<Option<Vec<u8>>, DHTError> {
        self.dht.get_record(key).await
    }

    /// Stores record `value` associated with `key`.
    pub async fn set_record(&self, key: Vec<u8>, value: Vec<u8>) -> Result<Vec<u8>, DHTError> {
        self.dht.set_record(key, value).await
    }

    /// Initializes and attempts to connect to the network.
    pub async fn connect(&mut self) -> Result<(), DHTError> {
        self.dht.start().await.and_then(|_| Ok(()))
    }

    /// Disconnect and deallocate connections to the network.
    pub async fn disconnect(&mut self) -> Result<(), DHTError> {
        self.dht.stop().await
    }
}
