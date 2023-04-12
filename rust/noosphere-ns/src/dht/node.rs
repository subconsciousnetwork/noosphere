use crate::dht::{
    channel::message_channel,
    errors::DhtError,
    processor::DhtProcessor,
    rpc::{DhtMessageClient, DhtRequest, DhtResponse},
    types::{DhtRecord, NetworkInfo, Peer},
    DhtConfig, Validator,
};
use libp2p::{identity::Keypair, Multiaddr, PeerId};
use std::time::Duration;
use tokio;

macro_rules! ensure_response {
    ($response:expr, $matcher:pat => $statement:expr) => {
        match $response {
            $matcher => $statement,
            _ => Err(DhtError::Error("Unexpected".into())),
        }
    };
}

/// A node that participates in a DHT network.
///
/// # Example
///
/// ```
/// use noosphere_ns::dht::{DhtConfig, DhtNode, Validator, AllowAllValidator};
/// use libp2p::{identity::Keypair, core::identity::ed25519, Multiaddr};
/// use std::str::FromStr;
/// use async_trait::async_trait;
/// use tokio;
///
/// #[tokio::main]
/// async fn main() {
///     let node = DhtNode::new(
///         Keypair::Ed25519(ed25519::Keypair::generate()),
///         Default::default(),
///         Some(AllowAllValidator{}),
///     ).unwrap();
///     node.listen("/ip4/127.0.0.1/tcp/0".parse().unwrap()).await.unwrap();
///     node.bootstrap().await.unwrap();
/// }
/// ```
pub struct DhtNode {
    config: DhtConfig,
    client: DhtMessageClient,
    thread_handle: tokio::task::JoinHandle<Result<(), DhtError>>,
    peer_id: PeerId,
}

impl DhtNode {
    pub fn new<V: Validator + 'static>(
        keypair: Keypair,
        config: DhtConfig,
        validator: Option<V>,
    ) -> Result<Self, DhtError> {
        let peer_id = PeerId::from(keypair.public());
        let channels = message_channel::<DhtRequest, DhtResponse, DhtError>();
        let thread_handle =
            DhtProcessor::spawn(&keypair, peer_id, validator, config.clone(), channels.1)?;

        Ok(DhtNode {
            peer_id,
            config,
            client: channels.0,
            thread_handle,
        })
    }

    /// Returns a reference to the [DHTConfig] used to
    /// initialize this node.
    pub fn config(&self) -> &DhtConfig {
        &self.config
    }

    /// Returns the [PeerId] of the current node.
    pub fn peer_id(&self) -> &PeerId {
        &self.peer_id
    }

    /// Returns the listening addresses of this node.
    pub async fn addresses(&self) -> Result<Vec<Multiaddr>, DhtError> {
        let request = DhtRequest::GetAddresses { external: false };
        let response = self.send_request(request).await?;
        ensure_response!(response, DhtResponse::GetAddresses(addresses) => Ok(addresses))
    }

    /// Returns the external listening addresses of this node, if any.
    pub async fn external_addresses(&self) -> Result<Vec<Multiaddr>, DhtError> {
        let request = DhtRequest::GetAddresses { external: false };
        let response = self.send_request(request).await?;
        ensure_response!(response, DhtResponse::GetAddresses(addresses) => Ok(addresses))
    }

    /// Adds additional peers to the DHT routing table. At least
    /// one peer is needed to connect to the network.
    pub async fn add_peers(&self, peers: Vec<Multiaddr>) -> Result<(), DhtError> {
        let request = DhtRequest::AddPeers { peers };
        let response = self.send_request(request).await?;
        ensure_response!(response, DhtResponse::Success => Ok(()))
    }

    /// Allow this node to act as a server node and listen
    /// for incoming connections on the provided [Multiaddr].
    pub async fn listen(&self, listening_address: Multiaddr) -> Result<Multiaddr, DhtError> {
        let request = DhtRequest::StartListening {
            address: listening_address,
        };
        let response = self.send_request(request).await?;
        ensure_response!(response, DhtResponse::Address(addr) => Ok(addr))
    }

    /// Stops listening on the provided address.
    pub async fn stop_listening(&self) -> Result<(), DhtError> {
        let request = DhtRequest::StopListening;
        let response = self.send_request(request).await?;
        ensure_response!(response, DhtResponse::Success => Ok(()))
    }

    /// Resolves once there are at least `requested_peers` peers
    /// in the network.
    pub async fn wait_for_peers(&self, requested_peers: usize) -> Result<(), DhtError> {
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
    pub async fn bootstrap(&self) -> Result<(), DhtError> {
        let request = DhtRequest::Bootstrap;
        let response = self.send_request(request).await?;
        ensure_response!(response, DhtResponse::Success => Ok(()))
    }

    /// Returns the current state of the network.
    /// Fails if node is not in an active state.
    pub async fn network_info(&self) -> Result<NetworkInfo, DhtError> {
        let request = DhtRequest::GetNetworkInfo;
        let response = self.send_request(request).await?;
        ensure_response!(response, DhtResponse::GetNetworkInfo(info) => Ok(info))
    }

    /// Returns the current state of the network.
    /// Fails if node is not in an active state.
    pub async fn peers(&self) -> Result<Vec<Peer>, DhtError> {
        let request = DhtRequest::GetPeers;
        let response = self.send_request(request).await?;
        ensure_response!(response, DhtResponse::GetPeers(peers) => Ok(peers))
    }

    /// Sets the record keyed by `key` with `value` and propagates
    /// to peers.
    /// Fails if node is not in an active state or cannot set the record
    /// on any peers.
    pub async fn put_record(&self, key: &[u8], value: &[u8]) -> Result<Vec<u8>, DhtError> {
        let request = DhtRequest::PutRecord {
            key: key.to_vec(),
            value: value.to_vec(),
        };
        let response = self.send_request(request).await?;
        ensure_response!(response, DhtResponse::PutRecord { key } => Ok(key))
    }

    /// Fetches the record keyed by `key` from the network.
    /// Return value may be `Ok(None)` if query finished without finding
    /// any matching values.
    /// Fails if node is not in an active state.
    pub async fn get_record(&self, key: &[u8]) -> Result<DhtRecord, DhtError> {
        let request = DhtRequest::GetRecord { key: key.to_vec() };
        let response = self.send_request(request).await?;
        ensure_response!(response, DhtResponse::GetRecord(record) => Ok(record))
    }

    /// Instructs the node to tell its peers that it is providing
    /// the record for `key`.
    /// Fails if node is not in an active state.
    pub async fn start_providing(&self, key: &[u8]) -> Result<(), DhtError> {
        let request = DhtRequest::StartProviding { key: key.to_vec() };
        let response = self.send_request(request).await?;
        ensure_response!(response, DhtResponse::Success => Ok(()))
    }

    /// Queries the network to find peers that are providing `key`.
    /// Fails if node is not in an active state.
    pub async fn get_providers(&self, key: &[u8]) -> Result<Vec<PeerId>, DhtError> {
        let request = DhtRequest::GetProviders { key: key.to_vec() };
        let response = self.send_request(request).await?;
        ensure_response!(response, DhtResponse::GetProviders { providers } => Ok(providers))
    }

    async fn send_request(&self, request: DhtRequest) -> Result<DhtResponse, DhtError> {
        self.client
            .send_request_async(request)
            .await
            .map_err(DhtError::from)
            .and_then(|res| res)
    }
}

impl Drop for DhtNode {
    fn drop(&mut self) {
        self.thread_handle.abort();
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod test {
    use super::*;
    use std::fmt::Display;

    use crate::dht::{AllowAllValidator, DhtError, DhtNode, NetworkInfo, Validator};
    use async_trait::async_trait;

    use crate::utils::make_p2p_address;
    use futures::future::try_join_all;
    use libp2p::{self, Multiaddr};
    use std::future::Future;
    use std::time::Duration;

    pub async fn wait_ms(ms: u64) {
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }

    async fn await_or_timeout<T>(
        timeout_ms: u64,
        future: impl Future<Output = T>,
        message: String,
    ) -> T {
        tokio::select! {
            _ = wait_ms(timeout_ms) => { panic!("timed out: {}", message); }
            result = future => { result }
        }
    }

    pub async fn swarm_command<'a, TFuture, F, T, E>(
        nodes: &'a mut [DhtNode],
        func: F,
    ) -> Result<Vec<T>, E>
    where
        F: FnMut(&'a mut DhtNode) -> TFuture,
        TFuture: Future<Output = Result<T, E>>,
    {
        let futures: Vec<_> = nodes.iter_mut().map(func).collect();
        try_join_all(futures).await
    }

    /// Creates a network of `node_count` nodes, with all nodes
    /// initially peering to the first created node, the "bootstrap".
    async fn create_network<V: Validator + Clone + 'static>(
        node_count: usize,
        validator: Option<V>,
    ) -> Result<Vec<DhtNode>, anyhow::Error> {
        let mut bootstrap_addresses: Option<Vec<Multiaddr>> = None;
        let mut nodes = vec![];
        for _ in 0..node_count {
            let node = DhtNode::new(
                Keypair::Ed25519(libp2p::core::identity::ed25519::Keypair::generate()),
                Default::default(),
                validator.clone(),
            )?;

            if let Some(addresses) = bootstrap_addresses.as_ref() {
                // Calling `add_peers()` before `listen()` is necessary, otherwise
                // the initial peering takes longer, triggering intermittent
                // timeouts in our tests (#311).
                node.add_peers(addresses.to_owned()).await?;
                node.listen("/ip4/127.0.0.1/tcp/0".parse().unwrap()).await?;
            } else {
                let address = node.listen("/ip4/127.0.0.1/tcp/0".parse().unwrap()).await?;
                bootstrap_addresses = Some(vec![address]);
            }
            nodes.push(node);
        }
        Ok(nodes)
    }

    async fn initialize_network(nodes: &mut Vec<DhtNode>) -> Result<(), anyhow::Error> {
        let expected_peers = nodes.len() - 1;
        // Wait a few, since nodes need to announce each other via Identify,
        // which adds their address to the routing table. Kick off
        // another bootstrap process after that.
        // @TODO Figure out if bootstrapping is needed after identify-exchange,
        // as that typically happens on a ~5 minute timer.
        wait_ms(700).await;
        swarm_command(nodes, |c| c.bootstrap()).await?;

        // Wait for the peers to establish connections.
        await_or_timeout(
            5000,
            swarm_command(nodes, |c| c.wait_for_peers(expected_peers)),
            format!("waiting for {} peers", expected_peers),
        )
        .await?;
        Ok(())
    }

    fn create_unfiltered_dht_node() -> Result<DhtNode, DhtError> {
        DhtNode::new::<AllowAllValidator>(
            Keypair::Ed25519(libp2p::core::identity::ed25519::Keypair::generate()),
            Default::default(),
            Some(AllowAllValidator {}),
        )
    }

    /// Testing a detached DHTNode as a server with no peers.
    #[tokio::test]
    async fn test_dhtnode_base_case() -> Result<(), DhtError> {
        let node = create_unfiltered_dht_node()?;
        node.listen("/ip4/127.0.0.1/tcp/0".parse().unwrap()).await?;
        let info = node.network_info().await?;
        assert_eq!(
            info,
            NetworkInfo {
                num_connections: 0,
                num_established: 0,
                num_peers: 0,
                num_pending: 0,
            }
        );

        if node.bootstrap().await.is_err() {
            panic!("bootstrap() should succeed, even without peers to bootstrap.");
        }
        Ok(())
    }

    /// Tests many nodes connecting to a single bootstrap node,
    /// and ensuring the nodes become peers.
    #[tokio::test]
    async fn test_dhtnode_bootstrap() -> Result<(), DhtError> {
        let num_nodes = 5;
        let mut nodes = create_network(num_nodes, Some(AllowAllValidator {})).await?;
        initialize_network(&mut nodes).await?;

        for info in swarm_command(&mut nodes, |c| c.network_info()).await? {
            assert_eq!(info.num_peers, num_nodes - 1);
            // TODO(#100) the number of connections seem inconsistent??
            //assert_eq!(info.num_connections, num_clients as u32);
            //assert_eq!(info.num_established, num_clients as u32);
            assert_eq!(info.num_pending, 0);
        }

        let info = nodes.first().unwrap().network_info().await?;
        assert_eq!(info.num_peers, num_nodes - 1);
        // TODO(#100) the number of connections seem inconsistent??
        //assert_eq!(info.num_connections, num_clients as u32);
        //assert_eq!(info.num_established, num_clients as u32);
        assert_eq!(info.num_pending, 0);

        Ok(())
    }

    /// Testing primitive set_record/get_record.
    #[tokio::test]
    async fn test_dhtnode_simple() -> Result<(), DhtError> {
        let mut nodes = create_network(2, Some(AllowAllValidator {})).await?;
        initialize_network(&mut nodes).await?;
        let (node_a, node_b) = (nodes.pop().unwrap(), nodes.pop().unwrap());

        node_a.put_record(b"foo", b"bar").await?;
        let result = node_b.get_record(b"foo").await?;
        assert_eq!(result.key, b"foo");
        assert_eq!(result.value.expect("has value"), b"bar");
        Ok(())
    }

    /// Testing primitive start_providing/get_providers.
    #[tokio::test]
    async fn test_dhtnode_providers() -> Result<(), DhtError> {
        let mut nodes = create_network(2, Some(AllowAllValidator {})).await?;
        initialize_network(&mut nodes).await?;
        let (node_a, node_b) = (nodes.pop().unwrap(), nodes.pop().unwrap());

        node_a.start_providing(b"foo").await?;

        let providers = node_b.get_providers(b"foo").await?;
        assert_eq!(providers.len(), 1);
        assert_eq!(&providers[0], node_a.peer_id());
        Ok(())
    }

    #[tokio::test]
    async fn test_dhtnode_validator() -> Result<(), DhtError> {
        #[derive(Clone)]
        struct MyValidator {}

        #[async_trait]
        impl Validator for MyValidator {
            async fn validate(&mut self, data: &[u8]) -> bool {
                data == b"VALID"
            }
        }

        impl Display for MyValidator {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "MyValidator")
            }
        }

        let mut nodes = create_network(2, Some(MyValidator {})).await?;
        initialize_network(&mut nodes).await?;
        let (node_a, node_b) = (nodes.pop().unwrap(), nodes.pop().unwrap());
        let unfiltered_client = create_unfiltered_dht_node()?;
        unfiltered_client
            .add_peers(vec![make_p2p_address(
                node_a.addresses().await?.pop().unwrap(),
                node_a.peer_id().to_owned(),
            )])
            .await?;

        node_a.put_record(b"foo_1", b"VALID").await?;
        let result = node_b.get_record(b"foo_1").await?;
        assert_eq!(
            result.value.expect("has value"),
            b"VALID",
            "validation allows valid records through"
        );

        assert!(
            node_a.put_record(b"foo_2", b"INVALID").await.is_err(),
            "setting a record validates locally"
        );

        // set a valid and an invalid record from the unfiltered client
        unfiltered_client.put_record(b"foo_3", b"VALID").await?;
        unfiltered_client.put_record(b"foo_4", b"INVALID").await?;

        let result = node_b.get_record(b"foo_3").await?;
        assert_eq!(
            result.value.expect("has value"),
            b"VALID",
            "validation allows valid records through"
        );

        assert!(
            node_b.get_record(b"foo_4").await?.value.is_none(),
            "invalid records are not retrieved from the network"
        );

        Ok(())
    }
}
