use crate::dht::{
    channel::message_channel,
    errors::DHTError,
    keys::DHTKeyMaterial,
    processor::DHTProcessor,
    rpc::{DHTMessageClient, DHTNetworkInfo, DHTRecord, DHTRequest, DHTResponse},
    DHTConfig, RecordValidator,
};
use libp2p::{multiaddr::Protocol, Multiaddr, PeerId};
use std::time::Duration;
use tokio;

macro_rules! ensure_response {
    ($response:expr, $matcher:pat => $statement:expr) => {
        match $response {
            $matcher => $statement,
            _ => Err(DHTError::Error("Unexpected".into())),
        }
    };
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DHTStatus {
    Initialized,
    Active,
    Terminated,
    Error(String),
}

/// A node that participates in a DHT network.
///
/// # Example
///
/// ```
/// use noosphere_ns::dht::{RecordValidator, DHTConfig, DHTNode};
/// use noosphere_core::authority::generate_ed25519_key;
/// use libp2p::{self, Multiaddr};
/// use std::str::FromStr;
/// use async_trait::async_trait;
/// use tokio;
///
/// #[derive(Clone)]
/// struct NoopValidator {}
///
/// #[async_trait]
/// impl RecordValidator for NoopValidator {
///     async fn validate(&mut self, data: &[u8]) -> bool {
///         true
///     }
/// }
///
/// #[tokio::main]
/// async fn main() {
///     // Note: not a real bootstrap node
///     let bootstrap_peers: Vec<Multiaddr> = vec!["/ip4/127.0.0.50/tcp/33333/p2p/12D3KooWH8WgH9mgbMXrKX4veokUznvEn6Ycwg4qaGNi83nLkoUK".parse().unwrap()];
///     let key = generate_ed25519_key();
///     let config = DHTConfig::default();
///     let validator = NoopValidator {};
///
///     let mut node = DHTNode::new(&key, DHTConfig::default(), Some(validator)).unwrap();
///     node.add_peers(bootstrap_peers).await.unwrap();
///     node.start_listening("/ip4/127.0.0.1/tcp/0".parse().unwrap()).await.unwrap();
///     node.bootstrap().await.unwrap();
/// }
/// ```
pub struct DHTNode {
    config: DHTConfig,
    client: DHTMessageClient,
    thread_handle: tokio::task::JoinHandle<Result<(), DHTError>>,
    peer_id: PeerId,
}

impl DHTNode {
    pub fn new<K: DHTKeyMaterial, V: RecordValidator + 'static>(
        key_material: &K,
        config: DHTConfig,
        validator: Option<V>,
    ) -> Result<Self, DHTError> {
        let keypair = key_material.to_dht_keypair()?;
        let peer_id = PeerId::from(keypair.public());

        let channels = message_channel::<DHTRequest, DHTResponse, DHTError>();
        let thread_handle = DHTProcessor::spawn(
            &keypair,
            peer_id.clone(),
            validator,
            config.clone(),
            channels.1,
        )?;

        Ok(DHTNode {
            peer_id,
            config,
            client: channels.0,
            thread_handle,
        })
    }

    /// Returns a reference to the [DHTConfig] used to
    /// initialize this node.
    pub fn config(&self) -> &DHTConfig {
        &self.config
    }

    /// Returns the [PeerId] of the current node.
    pub fn peer_id(&self) -> &PeerId {
        &self.peer_id
    }

    /// Returns the listening addresses of this node.
    pub async fn addresses(&self) -> Result<Vec<Multiaddr>, DHTError> {
        let request = DHTRequest::GetAddresses { external: false };
        let response = self.send_request(request).await?;
        ensure_response!(response, DHTResponse::GetAddresses(addresses) => Ok(addresses))
    }

    /// Returns the listening addresses of this node as a P2P address.
    pub async fn p2p_addresses(&self) -> Result<Vec<Multiaddr>, DHTError> {
        let peer_id = self.peer_id();
        Ok(self
            .addresses()
            .await?
            .into_iter()
            .map(|mut addr| {
                addr.push(Protocol::P2p(peer_id.to_owned().into()));
                addr
            })
            .collect::<Vec<Multiaddr>>())
    }

    /// Adds additional peers to the DHT routing table. At least
    /// one peer is needed to connect to the network.
    pub async fn add_peers(&self, peers: Vec<Multiaddr>) -> Result<(), DHTError> {
        let request = DHTRequest::AddPeers { peers };
        let response = self.send_request(request).await?;
        ensure_response!(response, DHTResponse::Success => Ok(()))
    }

    /// Allow this node to act as a server node and listen
    /// for incoming connections on the provided [Multiaddr].
    pub async fn start_listening(&self, listening_address: Multiaddr) -> Result<(), DHTError> {
        let request = DHTRequest::StartListening {
            address: listening_address,
        };
        let response = self.send_request(request).await?;
        ensure_response!(response, DHTResponse::Success => Ok(()))
    }

    /// Stops listening on the provided address.
    pub async fn stop_listening(&self, listening_address: Multiaddr) -> Result<(), DHTError> {
        let request = DHTRequest::StopListening {
            address: listening_address,
        };
        let response = self.send_request(request).await?;
        ensure_response!(response, DHTResponse::Success => Ok(()))
    }

    /// Resolves once there are at least `requested_peers` peers
    /// in the network.
    pub async fn wait_for_peers(&self, requested_peers: usize) -> Result<(), DHTError> {
        // TODO(#101) Need to add a mechanism for non-Query based requests,
        // like sending events, or triggering a peer check on
        // new connection established. For now, we poll here.
        loop {
            let info = self.network_info().await?;
            if info.num_peers >= requested_peers {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    /// Instructs the node to initiate the bootstrap process,
    /// resolving once the process begins successfully.
    /// Generally, this method is usually not necessary, as nodes
    /// automatically bootstrap themselves.
    /// Fails if node is not in an active state, or bootstrapping
    /// unable to start.
    pub async fn bootstrap(&self) -> Result<(), DHTError> {
        let request = DHTRequest::Bootstrap;
        let response = self.send_request(request).await?;
        ensure_response!(response, DHTResponse::Success => Ok(()))
    }

    /// Returns the current state of the network.
    /// Fails if node is not in an active state.
    pub async fn network_info(&self) -> Result<DHTNetworkInfo, DHTError> {
        let request = DHTRequest::GetNetworkInfo;
        let response = self.send_request(request).await?;
        ensure_response!(response, DHTResponse::GetNetworkInfo(info) => Ok(info))
    }

    /// Sets the record keyed by `key` with `value` and propagates
    /// to peers.
    /// Fails if node is not in an active state or cannot set the record
    /// on any peers.
    pub async fn put_record(&self, key: &[u8], value: &[u8]) -> Result<Vec<u8>, DHTError> {
        let request = DHTRequest::PutRecord {
            key: key.to_vec(),
            value: value.to_vec(),
        };
        let response = self.send_request(request).await?;
        ensure_response!(response, DHTResponse::PutRecord { key } => Ok(key))
    }

    /// Fetches the record keyed by `key` from the network.
    /// Return value may be `Ok(None)` if query finished without finding
    /// any matching values.
    /// Fails if node is not in an active state.
    pub async fn get_record(&self, key: &[u8]) -> Result<DHTRecord, DHTError> {
        let request = DHTRequest::GetRecord { key: key.to_vec() };
        let response = self.send_request(request).await?;
        ensure_response!(response, DHTResponse::GetRecord(record) => Ok(record))
    }

    /// Instructs the node to tell its peers that it is providing
    /// the record for `key`.
    /// Fails if node is not in an active state.
    pub async fn start_providing(&self, key: &[u8]) -> Result<(), DHTError> {
        let request = DHTRequest::StartProviding { key: key.to_vec() };
        let response = self.send_request(request).await?;
        ensure_response!(response, DHTResponse::Success => Ok(()))
    }

    /// Queries the network to find peers that are providing `key`.
    /// Fails if node is not in an active state.
    pub async fn get_providers(&self, key: &[u8]) -> Result<Vec<PeerId>, DHTError> {
        let request = DHTRequest::GetProviders { key: key.to_vec() };
        let response = self.send_request(request).await?;
        ensure_response!(response, DHTResponse::GetProviders { providers } => Ok(providers))
    }

    async fn send_request(&self, request: DHTRequest) -> Result<DHTResponse, DHTError> {
        self.client
            .send_request_async(request)
            .await
            .map_err(DHTError::from)
            .and_then(|res| res)
    }
}

impl Drop for DHTNode {
    fn drop(&mut self) {
        self.thread_handle.abort();
    }
}
