use crate::dht::{
    channel::message_channel,
    errors::DHTError,
    node::DHTNode,
    types::{DHTMessageClient, DHTNetworkInfo, DHTRequest, DHTResponse, DHTStatus},
    DHTConfig,
};
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

/// Represents a DHT node running in a network thread, providing
/// async methods for operating the node. Use [spawn_dht_node] to
/// spawn a new DHT node, returning the corresponding [DHTNodeHandle].
pub struct DHTNodeHandle {
    config: DHTConfig,
    state: DHTStatus,
    client: Option<DHTMessageClient>,
    thread_handle: Option<tokio::task::JoinHandle<Result<(), DHTError>>>,
}

impl DHTNodeHandle {
    pub(crate) fn new(config: DHTConfig) -> Result<Self, DHTError> {
        let (client, processor) = message_channel::<DHTRequest, DHTResponse, DHTError>();
        let thread_handle = DHTNode::spawn(config.clone(), processor)?;

        Ok(DHTNodeHandle {
            config,
            state: DHTStatus::Active,
            client: Some(client),
            thread_handle: Some(thread_handle),
        })
    }

    /// Teardown the network processing thread.
    /// @TODO Anything to wait on for thread cleanup?
    pub fn terminate(&mut self) -> Result<(), DHTError> {
        self.ensure_state(DHTStatus::Active)?;
        self.state = DHTStatus::Terminated;
        self.thread_handle
            .take()
            .expect("active DHTNodeHandles must have thread handle")
            .abort();
        self.client = None;
        Ok(())
    }

    /// Returns the public address of this node.
    /// @TODO Need to untangle how libp2p manages
    /// local vs remote address representation.
    pub fn p2p_address(&self) -> libp2p::Multiaddr {
        self.config.p2p_address()
    }

    pub fn status(&self) -> DHTStatus {
        self.state.clone()
    }

    /// Resolves once there are at least `requested_peers` peers
    /// in the network.
    pub async fn wait_for_peers(&self, requested_peers: usize) -> Result<(), DHTError> {
        // Need to add a mechanism for non-Query based requests,
        // like sending events, or triggering a peer check on
        // new connection established.
        // For now, we poll here.
        warn!("WAIT FOR PEERS: {:#?}", requested_peers);
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
        let client = self.client.as_ref().unwrap();
        client
            .send_request_async(request)
            .await
            .map_err(|e| DHTError::from(e))
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

impl Drop for DHTNodeHandle {
    fn drop(&mut self) {
        debug!("Dropping DHTClient");
    }
}
