use crate::dht::{
    channel::message_channel,
    errors::DHTError,
    processor::DHTProcessor,
    types::{DHTMessageClient, DHTNetworkInfo, DHTRequest, DHTResponse},
    utils::key_material_to_libp2p_keypair,
    DHTConfig,
};
use std::{net::SocketAddr, time::Duration};
use tokio;
use ucan_key_support::ed25519::Ed25519KeyMaterial;

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

/// Represents a DHT node running in a network thread, providing
/// async methods for operating the node.
pub struct DHTNode {
    config: DHTConfig,
    state: DHTStatus,
    client: Option<DHTMessageClient>,
    thread_handle: Option<tokio::task::JoinHandle<Result<(), DHTError>>>,
    keypair: libp2p::identity::Keypair,
    peer_id: libp2p::PeerId,
    p2p_address: libp2p::Multiaddr,
    bootstrap_peers: Option<Vec<libp2p::Multiaddr>>,
}

impl DHTNode {
    /// Creates a new [DHTNode].
    /// `listening_address` is a [std::net::SocketAddr] that is used to listen for incoming
    /// connections.
    /// `bootstrap_peers` is a collection of [String]s in [libp2p::Multiaddr] form of initial
    /// peers to connect to during bootstrapping. This collection would be empty in the
    /// standalone bootstrap node scenario.
    pub fn new(
        key_material: &Ed25519KeyMaterial,
        listening_address: &SocketAddr,
        bootstrap_peers: Option<&Vec<String>>,
        config: &DHTConfig,
    ) -> Result<Self, DHTError> {
        let keypair = key_material_to_libp2p_keypair(key_material)?;
        let peer_id = libp2p::PeerId::from(keypair.public());
        let p2p_address = {
            let mut multiaddr: libp2p::Multiaddr = listening_address.ip().into();
            multiaddr.push(libp2p::multiaddr::Protocol::Tcp(listening_address.port()));
            multiaddr.push(libp2p::multiaddr::Protocol::P2p(peer_id.into()));
            multiaddr
        };

        let peers: Option<Vec<libp2p::Multiaddr>> = if let Some(peers) = bootstrap_peers {
            Some(
                peers
                    .iter()
                    .map(|p| p.parse())
                    .collect::<Result<Vec<libp2p::Multiaddr>, libp2p::multiaddr::Error>>()
                    .map_err(|e| DHTError::Error(e.to_string()))?,
            )
        } else {
            None
        };

        Ok(DHTNode {
            keypair,
            peer_id,
            p2p_address,
            config: config.to_owned(),
            bootstrap_peers: peers,
            state: DHTStatus::Initialized,
            client: None,
            thread_handle: None,
        })
    }

    /// Start the DHT network.
    pub fn run(&mut self) -> Result<(), DHTError> {
        let (client, processor) = message_channel::<DHTRequest, DHTResponse, DHTError>();
        self.ensure_state(DHTStatus::Initialized)?;
        self.client = Some(client);
        self.thread_handle = Some(DHTProcessor::spawn(
            &self.keypair,
            &self.peer_id,
            &self.p2p_address,
            &self.bootstrap_peers,
            &self.config,
            processor,
        )?);
        self.state = DHTStatus::Active;
        Ok(())
    }

    /// Teardown the network processing thread.
    pub fn terminate(&mut self) -> Result<(), DHTError> {
        self.ensure_state(DHTStatus::Active)?;
        if let Some(thread_handle) = self.thread_handle.take() {
            thread_handle.abort();
        }
        self.state = DHTStatus::Terminated;
        Ok(())
    }

    /// Adds additional bootstrap peers. Can only be executed before calling [DHTNode::run].
    pub fn add_peers(&mut self, new_peers: &Vec<String>) -> Result<(), DHTError> {
        self.ensure_state(DHTStatus::Initialized)?;
        let mut new_peers_list: Vec<libp2p::Multiaddr> = new_peers
            .iter()
            .map(|p| p.parse())
            .collect::<Result<Vec<libp2p::Multiaddr>, libp2p::multiaddr::Error>>()
            .map_err(|e| DHTError::Error(e.to_string()))?;

        if let Some(ref mut peers) = self.bootstrap_peers {
            peers.append(&mut new_peers_list);
        } else {
            self.bootstrap_peers = Some(new_peers_list);
        }
        Ok(())
    }

    /// Returns a reference to the [DHTConfig] used to
    /// initialize this node.
    pub fn config(&self) -> &DHTConfig {
        &self.config
    }

    /// Returns the [libp2p::PeerId] of the current node.
    pub fn peer_id(&self) -> &libp2p::PeerId {
        &self.peer_id
    }

    /// Returns the listening address of this node.
    pub fn p2p_address(&self) -> &libp2p::Multiaddr {
        &self.p2p_address
    }

    pub fn status(&self) -> DHTStatus {
        self.state.clone()
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

    /// Sets the record keyed by `name` with `value` and propagates
    /// to peers.
    /// Fails if node is not in an active state or cannot set the record
    /// on any peers.
    pub async fn set_record(&self, name: Vec<u8>, value: Vec<u8>) -> Result<Vec<u8>, DHTError> {
        let request = DHTRequest::SetRecord { name, value };
        let response = self.send_request(request).await?;
        ensure_response!(response, DHTResponse::SetRecord { name } => Ok(name))
    }

    /// Fetches the record keyed by `name` from the network.
    /// Return value may be `Ok(None)` if query finished without finding
    /// any matching values.
    /// Fails if node is not in an active state.
    pub async fn get_record(&self, name: Vec<u8>) -> Result<Option<Vec<u8>>, DHTError> {
        let request = DHTRequest::GetRecord { name };
        let response = self.send_request(request).await?;
        ensure_response!(response, DHTResponse::GetRecord { value, .. } => Ok(Some(value)))
    }

    /// Instructs the node to tell its peers that it is providing
    /// the record for `name`.
    /// Fails if node is not in an active state.
    pub async fn start_providing(&self, name: Vec<u8>) -> Result<(), DHTError> {
        let request = DHTRequest::StartProviding { name };
        let response = self.send_request(request).await?;
        ensure_response!(response, DHTResponse::StartProviding { name: _ } => Ok(()))
    }

    /// Queries the network to find peers that are providing `name`.
    /// Fails if node is not in an active state.
    pub async fn get_providers(&self, name: Vec<u8>) -> Result<Vec<libp2p::PeerId>, DHTError> {
        let request = DHTRequest::GetProviders { name };
        let response = self.send_request(request).await?;
        ensure_response!(response, DHTResponse::GetProviders { providers, name: _ } => Ok(providers))
    }

    async fn send_request(&self, request: DHTRequest) -> Result<DHTResponse, DHTError> {
        self.ensure_state(DHTStatus::Active)?;
        self.client
            .as_ref()
            .expect("active DHT has client")
            .send_request_async(request)
            .await
            .map_err(DHTError::from)
            .and_then(|res| res)
    }

    /// Returns `Ok(())` if current status matches expected status.
    /// Otherwise, returns a [DHTError].
    fn ensure_state(&self, expected_status: DHTStatus) -> Result<(), DHTError> {
        if self.state != expected_status {
            if expected_status == DHTStatus::Active {
                Err(DHTError::NotConnected)
            } else {
                Err(DHTError::Error("invalid state".into()))
            }
        } else {
            Ok(())
        }
    }
}
