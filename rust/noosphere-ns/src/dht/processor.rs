use crate::dht::{
    errors::DHTError,
    swarm::{build_swarm, DHTEvent, DHTSwarm, DHTSwarmEvent},
    types::{DHTMessage, DHTMessageProcessor, DHTRequest, DHTResponse},
    DHTConfig,
};
use libp2p::{
    futures::StreamExt,
    identify::Event as IdentifyEvent,
    kad::{
        self,
        kbucket::{Distance, NodeStatus},
        record::{store::RecordStore, Key},
        KademliaEvent, PeerRecord, QueryResult, Quorum, Record,
    },
    swarm::{
        dial_opts::{DialOpts, PeerCondition},
        SwarmEvent,
    },
    PeerId,
};
use std::fmt;
use std::{collections::HashMap, time::Duration};
use tokio;

/// The processing component of a [DHTNode]/[DHTProcessor] pair. Consumers
/// should only interface with a [DHTProcessor] via [DHTNode].
pub struct DHTProcessor {
    config: DHTConfig,
    peer_id: PeerId,
    p2p_address: Option<libp2p::Multiaddr>,
    processor: DHTMessageProcessor,
    swarm: DHTSwarm,
    requests: HashMap<kad::QueryId, DHTMessage>,
    kad_last_range: Option<(Distance, Distance)>,
}

// Temporary(?), exploring processing both requests that
// are bound by kad::QueryId, and requests that do not tie
// into DHT queries (like WaitForPeers).
macro_rules! store_request {
    ($self:expr, $message:expr, $result:expr) => {
        let result: Result<kad::QueryId, DHTError> = $result.map_err(|e| e.into());
        match result {
            Ok(query_id) => {
                $self.requests.insert(query_id, $message);
            }
            Err(e) => {
                $message.respond(Err(e));
            }
        };
    };
}

impl DHTProcessor {
    /// Creates a new [DHTProcessor] and spawns a networking thread for processing.
    /// The processor can only be accessed through channels via the corresponding
    /// [DHTNode].
    pub(crate) fn spawn(
        keypair: &libp2p::identity::Keypair,
        peer_id: &PeerId,
        p2p_address: &Option<libp2p::Multiaddr>,
        bootstrap_peers: &Option<Vec<libp2p::Multiaddr>>,
        config: &DHTConfig,
        processor: DHTMessageProcessor,
    ) -> Result<tokio::task::JoinHandle<Result<(), DHTError>>, DHTError> {
        let swarm = build_swarm(keypair, peer_id, config)?;
        let peers = bootstrap_peers.to_owned();

        let mut node = DHTProcessor {
            peer_id: peer_id.to_owned(),
            p2p_address: p2p_address.to_owned(),
            config: config.to_owned(),
            processor,
            swarm,
            requests: HashMap::default(),
            kad_last_range: None,
        };

        Ok(tokio::spawn(async move {
            node.initialize(peers).await;
            node.process().await
        }))
    }

    /// Sets up the [DHTProcessor]. Adds bootstrap peers to the routing table.
    async fn initialize(&mut self, bootstrap_peers: Option<Vec<libp2p::Multiaddr>>) {
        // Add the bootnodes to the local routing table.
        if let Some(peers) = bootstrap_peers {
            for multiaddress in peers {
                let mut addr = multiaddress.to_owned();
                if let Some(libp2p::multiaddr::Protocol::P2p(p2p_hash)) = addr.pop() {
                    let peer_id = PeerId::from_multihash(p2p_hash).unwrap();
                    // Do not add a peer with the same peer id, for example
                    // a set of N bootstrap nodes using a static list of
                    // N addresses/peer IDs.
                    if peer_id != self.peer_id {
                        self.swarm.behaviour_mut().kad.add_address(&peer_id, addr);
                    }
                }
            }
        }
    }

    /// Begin processing requests and connections on the DHT network
    /// in the current thread. Executes until the loop is broken, via
    /// either an unhandlable error or a terminate message (not yet implemented).
    async fn process(&mut self) -> Result<(), DHTError> {
        self.start_listening()?;

        // Queue up bootstrapping this node both immediately, and every
        // `bootstrap_interval` seconds.
        let mut bootstrap_tick =
            tokio::time::interval(Duration::from_secs(self.config.bootstrap_interval));

        // Traverse and potentially dial peers on this interval.
        let mut peer_dialing_tick =
            tokio::time::interval(Duration::from_secs(self.config.peer_dialing_interval));

        loop {
            tokio::select! {
                message = self.processor.pull_message() => {
                    match message {
                        Some(m) => self.process_message(m),
                        // This occurs when sender is closed (client dropped).
                        // Exit the process loop for thread clean up.
                        None => {
                            error!("DHT processing loop unexpectedly closed.");
                            break
                        },
                    }
                }
                event = self.swarm.select_next_some() => {
                    self.process_swarm_event(event)
                }
                _ = bootstrap_tick.tick() => self.execute_bootstrap()?,
                _ = peer_dialing_tick.tick() => self.dial_next_peer(),
            }
        }
        Ok(())
    }

    /// Processes an incoming DHTMessage. Will attempt to respond
    /// immediately if possible (synchronous error or pulling value from cache),
    /// otherwise, the message will be mapped to a query, where it can be fulfilled
    /// later, most likely in `process_kad_result()`.
    fn process_message(&mut self, message: DHTMessage) {
        dht_event_trace(self, &message);

        let behaviour = self.swarm.behaviour_mut();

        // Process client requests.
        match message.request {
            DHTRequest::GetProviders { ref name } => {
                store_request!(
                    self,
                    message,
                    Ok::<kad::QueryId, DHTError>(behaviour.kad.get_providers(Key::new(name)))
                );
            }
            /*
            DHTRequest::WaitForPeers(peers) => {
                let info = self.swarm.network_info();
                if info.num_peers() >= peers {
                    message.respond(Ok(DHTResponse::Success));
                } else {
                    store_request!(self, message);
                }
            }
            */
            DHTRequest::Bootstrap => {
                message.respond(self.execute_bootstrap().map(|_| DHTResponse::Success));
            }
            DHTRequest::GetNetworkInfo => {
                let info = self.swarm.network_info();
                message.respond(Ok(DHTResponse::GetNetworkInfo(info.into())));
            }
            DHTRequest::StartProviding { ref name } => {
                store_request!(self, message, behaviour.kad.start_providing(Key::new(name)));
            }
            DHTRequest::GetRecord { ref name } => {
                store_request!(
                    self,
                    message,
                    Ok::<kad::QueryId, DHTError>(
                        behaviour.kad.get_record(Key::new(name), Quorum::One)
                    )
                );
            }
            DHTRequest::SetRecord {
                ref name,
                ref value,
            } => {
                let record = Record {
                    key: Key::new(name),
                    value: value.clone(),
                    publisher: None,
                    expires: None,
                };
                info!("PUTTING RECORD {:#?}", record);
                store_request!(self, message, behaviour.kad.put_record(record, Quorum::One));
            }
        };
    }

    /// Processes an incoming SwarmEvent, triggered from swarm activity or
    /// a swarm query. If a SwarmEvent has an associated DHTQuery,
    /// the pending query will be fulfilled.
    fn process_swarm_event(&mut self, event: DHTSwarmEvent) {
        dht_event_trace(self, &event);
        match event {
            SwarmEvent::Behaviour(DHTEvent::Kademlia(e)) => self.process_kad_event(e),
            SwarmEvent::Behaviour(DHTEvent::Identify(e)) => self.process_identify_event(e),
            // The following events are currently handled only for debug logging.
            SwarmEvent::NewListenAddr { address: _, .. } => {}
            SwarmEvent::ConnectionEstablished { peer_id: _, .. } => {}
            SwarmEvent::ConnectionClosed {
                peer_id: _,
                cause: _,
                ..
            } => {}
            SwarmEvent::IncomingConnection {
                local_addr: _,
                send_back_addr: _,
            } => {}
            SwarmEvent::IncomingConnectionError {
                local_addr: _,
                send_back_addr: _,
                error: _,
            } => {}
            SwarmEvent::OutgoingConnectionError {
                peer_id: _,
                error: _,
            } => {}
            SwarmEvent::BannedPeer { peer_id: _, .. } => {}
            SwarmEvent::ExpiredListenAddr {
                listener_id: _,
                address: _,
            } => {}
            SwarmEvent::ListenerClosed {
                listener_id: _,
                addresses: _,
                reason: _,
            } => {}
            SwarmEvent::ListenerError {
                listener_id: _,
                error: _,
            } => {}
            SwarmEvent::Dialing(_) => {}
        }
    }

    fn process_kad_event(&mut self, event: KademliaEvent) {
        match event {
            KademliaEvent::OutboundQueryCompleted { id, result, .. } => match result {
                QueryResult::GetRecord(Ok(ok)) => {
                    for PeerRecord {
                        record: Record { key, value, .. },
                        ..
                    } in ok.records
                    {
                        if let Some(message) = self.requests.remove(&id) {
                            message.respond(Ok(DHTResponse::GetRecord {
                                name: key.to_vec(),
                                value: Some(value),
                            }));
                        }
                    }
                }
                QueryResult::GetRecord(Err(e)) => {
                    if let Some(message) = self.requests.remove(&id) {
                        message.respond(Err(DHTError::from(e)));
                    }
                }
                QueryResult::PutRecord(Ok(kad::PutRecordOk { key })) => {
                    if let Some(message) = self.requests.remove(&id) {
                        message.respond(Ok(DHTResponse::SetRecord { name: key.to_vec() }));
                    }
                }
                QueryResult::PutRecord(Err(e)) => {
                    match e {
                        kad::PutRecordError::Timeout {
                            ref key,
                            quorum: _,
                            success: _,
                        }
                        | kad::PutRecordError::QuorumFailed {
                            ref key,
                            quorum: _,
                            success: _,
                        } => {
                            let record = self.swarm.behaviour_mut().kad.store_mut().get(key);
                            trace!("Has internal record? {:?}", record);
                        }
                    }
                    if let Some(message) = self.requests.remove(&id) {
                        message.respond(Err(DHTError::from(e)));
                    }
                }
                QueryResult::StartProviding(Ok(kad::AddProviderOk { key })) => {
                    if let Some(message) = self.requests.remove(&id) {
                        message.respond(Ok(DHTResponse::StartProviding { name: key.to_vec() }));
                    }
                }
                QueryResult::StartProviding(Err(e)) => {
                    if let Some(message) = self.requests.remove(&id) {
                        message.respond(Err(DHTError::from(e)));
                    }
                }
                QueryResult::GetProviders(Ok(kad::GetProvidersOk {
                    providers,
                    key,
                    closest_peers: _,
                })) => {
                    if let Some(message) = self.requests.remove(&id) {
                        message.respond(Ok(DHTResponse::GetProviders {
                            providers: providers.into_iter().collect(),
                            name: key.to_vec(),
                        }));
                    }
                }
                QueryResult::GetProviders(Err(e)) => {
                    if let Some(message) = self.requests.remove(&id) {
                        message.respond(Err(DHTError::from(e)));
                    }
                }
                QueryResult::Bootstrap(Ok(kad::BootstrapOk {
                    peer: _,
                    num_remaining: _,
                })) => {}
                QueryResult::Bootstrap(Err(kad::BootstrapError::Timeout {
                    peer: _,
                    num_remaining: _,
                })) => {}
                _ => {}
            },
            KademliaEvent::InboundRequest { request } => match request {
                kad::InboundRequest::FindNode {
                    num_closer_peers: _,
                } => {}
                kad::InboundRequest::GetProvider {
                    num_closer_peers: _,
                    num_provider_peers: _,
                } => {}
                kad::InboundRequest::AddProvider { record: _ } => {}
                kad::InboundRequest::GetRecord {
                    num_closer_peers: _,
                    present_locally: _,
                } => {}
                kad::InboundRequest::PutRecord { source, record, .. } => match record {
                    Some(rec) => {
                        if self
                            .swarm
                            .behaviour_mut()
                            .kad
                            .store_mut()
                            .put(rec.clone())
                            .is_err()
                        {
                            warn!("InboundRequest::PutRecord failed: {:?} {:?}", rec, source);
                        }
                    }
                    None => warn!("InboundRequest::PutRecord failed; empty record"),
                },
            },
            KademliaEvent::RoutingUpdated {
                peer: _,
                is_new_peer: _,
                addresses: _,
                ..
            } => {}
            KademliaEvent::UnroutablePeer { peer: _ } => {}
            KademliaEvent::RoutablePeer {
                peer: _,
                address: _,
            } => {}
            KademliaEvent::PendingRoutablePeer {
                peer: _,
                address: _,
            } => {}
        }
    }

    fn process_identify_event(&mut self, event: IdentifyEvent) {
        if let IdentifyEvent::Received { peer_id, info } = event {
            if info
                .protocols
                .iter()
                .any(|p| p.as_bytes() == kad::protocol::DEFAULT_PROTO_NAME)
            {
                for addr in &info.listen_addrs {
                    self.swarm
                        .behaviour_mut()
                        .kad
                        .add_address(&peer_id, addr.clone());
                }
            }
        }
    }

    /// Traverses the kbuckets to dial potential peers that
    /// are not yet connected. Implementation inspired by iroh:
    /// https://github.com/n0-computer/iroh/blob/main/iroh-p2p/src/node.rs
    fn dial_next_peer(&mut self) {
        let mut to_dial = None;
        for kbucket in self.swarm.behaviour_mut().kad.kbuckets() {
            if let Some(range) = self.kad_last_range {
                if kbucket.range() == range {
                    continue;
                }
            }

            // find the first disconnected node
            for entry in kbucket.iter() {
                if entry.status == NodeStatus::Disconnected {
                    let peer_id = entry.node.key.preimage();

                    let dial_opts = DialOpts::peer_id(*peer_id)
                        .condition(PeerCondition::Disconnected)
                        .addresses(entry.node.value.clone().into_vec())
                        .extend_addresses_through_behaviour()
                        .build();
                    to_dial = Some((dial_opts, kbucket.range()));
                    break;
                }
            }
        }

        if let Some((dial_opts, range)) = to_dial {
            if let Err(e) = self.swarm.dial(dial_opts) {
                warn!("failed to dial: {:?}", e);
            }
            self.kad_last_range = Some(range);
        }
    }

    fn start_listening(&mut self) -> Result<(), DHTError> {
        match self.p2p_address.as_ref() {
            Some(p2p_address) => {
                let addr = p2p_address.to_owned();
                dht_event_trace(self, &format!("Start listening on {}", addr));
                self.swarm
                    .listen_on(addr)
                    .map(|_| ())
                    .map_err(DHTError::from)
            }
            None => Ok(()),
        }
    }

    fn execute_bootstrap(&mut self) -> Result<(), DHTError> {
        dht_event_trace(self, &"Execute bootstrap");
        match self.swarm.behaviour_mut().kad.bootstrap() {
            Ok(_) => Ok(()),
            Err(_) => {
                // `NoKnownPeers` error is expected without any bootstrap peers.
                Ok(())
            }
        }
    }
}

impl fmt::Debug for DHTProcessor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DHTNode")
            .field("peer_id", &self.peer_id)
            .field("config", &self.config)
            .finish()
    }
}

// #[cfg(test)]
/// Logging utility. Unfortunately, integration tests do not work
/// with `#[cfg(test)]` to enable the option of rendering the full
/// peer id during non-testing (one process, one peer id) scenarios.
/// https://doc.rust-lang.org/book/ch11-03-test-organization.html
fn dht_event_trace<T: std::fmt::Debug>(processor: &DHTProcessor, data: &T) {
    // Convert a full PeerId to a shorter, more identifiable
    // string for comparison in logs during tests, where multiple nodes
    // are shared by a single process. All Ed25519 keys have
    // the prefix `12D3KooW`, so skip the commonalities and use
    // the next 6 characters for logging.
    let peer_id_b58 = processor.peer_id.to_base58();
    trace!(
        "\nFrom ..{:#?}..\n{:#?}",
        peer_id_b58.get(8..14).unwrap_or("INVALID PEER ID"),
        data
    );
}

/*
#[cfg(not(test))]
fn dht_event_trace<T: std::fmt::Debug>(processor: &DHTProcessor, data: &T) {
    trace!(
        "\nFrom ..{:#?}..\n{:#?}",
        processor.peer_id.to_base58(),
        data
    );
}
*/
