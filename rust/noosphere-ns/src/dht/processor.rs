use super::{
    errors::DhtError,
    rpc::{DhtMessage, DhtMessageProcessor, DhtRequest, DhtResponse},
    swarm::{build_swarm, DHTEvent, DHTSwarmEvent, DhtBehavior},
    types::{DhtRecord, Peer},
    DhtConfig, Validator,
};
use libp2p::{
    core::transport::ListenerId,
    futures::StreamExt,
    identify::Event as IdentifyEvent,
    identity::Keypair,
    kad::{
        self,
        kbucket::{Distance, NodeStatus},
        record::{store::RecordStore, Key},
        KademliaEvent, PeerRecord, QueryResult, Quorum, Record,
    },
    multiaddr::Protocol,
    swarm::{
        dial_opts::{DialOpts, PeerCondition},
        Swarm, SwarmEvent,
    },
    Multiaddr, PeerId,
};
use std::{collections::HashMap, time::Duration};
use std::{fmt, num::NonZeroUsize};
use tokio;

/// The processing component of a [DHTNode]/[DHTProcessor] pair. Consumers
/// should only interface with a [DHTProcessor] via [DHTNode].
pub struct DhtProcessor<V: Validator + 'static> {
    config: DhtConfig,
    peer_id: PeerId,
    processor: DhtMessageProcessor,
    swarm: Swarm<DhtBehavior>,
    requests: HashMap<kad::QueryId, DhtMessage>,
    kad_last_range: Option<(Distance, Distance)>,
    validator: Option<V>,
    active_listener: Option<ListenerId>,
    pending_listener_request: Option<DhtMessage>,
}

// Temporary(?), exploring processing both requests that
// are bound by kad::QueryId, and requests that do not tie
// into DHT queries (like WaitForPeers).
macro_rules! store_request {
    ($self:expr, $message:expr, $result:expr) => {
        let result: Result<kad::QueryId, DhtError> = $result.map_err(|e| e.into());
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

impl<V> DhtProcessor<V>
where
    V: Validator + 'static,
{
    /// Creates a new [DHTProcessor] and spawns a networking thread for processing.
    /// The processor can only be accessed through channels via the corresponding
    /// [DHTNode].
    pub(crate) fn spawn(
        keypair: &Keypair,
        peer_id: PeerId,
        validator: Option<V>,
        config: DhtConfig,
        processor: DhtMessageProcessor,
    ) -> Result<tokio::task::JoinHandle<Result<(), DhtError>>, DhtError> {
        let swarm = build_swarm(keypair, &peer_id, &config)?;

        let mut node = DhtProcessor {
            peer_id,
            config,
            processor,
            swarm,
            requests: HashMap::default(),
            active_listener: None,
            kad_last_range: None,
            validator,
            pending_listener_request: None,
        };

        Ok(tokio::spawn(async move { node.process().await }))
    }

    /// Begin processing requests and connections on the DHT network
    /// in the current thread. Executes until the loop is broken, via
    /// either an unhandlable error or a terminate message (not yet implemented).
    async fn process(&mut self) -> Result<(), DhtError> {
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
                        Some(m) => self.process_message(m).await,
                        // This occurs when sender is closed (client dropped).
                        // Exit the process loop for thread clean up.
                        None => {
                            error!("DHT processing loop unexpectedly closed.");
                            break
                        },
                    }
                }
                event = self.swarm.select_next_some() => {
                    self.process_swarm_event(event).await
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
    #[instrument(skip(self), level = "trace")]
    async fn process_message(&mut self, message: DhtMessage) {
        // Process client requests.
        match message.request {
            DhtRequest::AddPeers { ref peers } => {
                let result = self.add_peers(peers).await.map(|_| DhtResponse::Success);
                message.respond(result);
            }
            DhtRequest::StartListening { ref address } => {
                if let Err(e) = self.listen(address) {
                    message.respond(Err(e));
                } else {
                    if let Some(current_pending) = self.pending_listener_request.take() {
                        current_pending.respond(Err(DhtError::Error(String::from(
                            "Subsequent listener request overrides previous request.",
                        ))));
                    }
                    self.pending_listener_request = Some(message);
                }
            }
            DhtRequest::StopListening => {
                let result = self.stop_listening().map(|_| DhtResponse::Success);
                message.respond(result);
            }
            DhtRequest::GetAddresses { external } => {
                let listeners: Vec<Multiaddr> = if external {
                    self.get_external_addresses()
                } else {
                    self.get_addresses()
                };
                message.respond(Ok(DhtResponse::GetAddresses(listeners)));
            }
            DhtRequest::Bootstrap => {
                message.respond(self.execute_bootstrap().map(|_| DhtResponse::Success));
            }
            DhtRequest::GetProviders { ref key } => {
                store_request!(
                    self,
                    message,
                    Ok::<kad::QueryId, DhtError>(
                        self.swarm.behaviour_mut().kad.get_providers(Key::new(key))
                    )
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
            DhtRequest::GetNetworkInfo => {
                let info = self.swarm.network_info();
                message.respond(Ok(DhtResponse::GetNetworkInfo(info.into())));
            }
            DhtRequest::GetPeers => {
                let peers = self
                    .swarm
                    .connected_peers()
                    .map(|peer_id| Peer {
                        peer_id: peer_id.to_owned(),
                    })
                    .collect();
                message.respond(Ok(DhtResponse::GetPeers(peers)));
            }
            DhtRequest::StartProviding { ref key } => {
                store_request!(
                    self,
                    message,
                    self.swarm
                        .behaviour_mut()
                        .kad
                        .start_providing(Key::new(key))
                );
            }
            DhtRequest::GetRecord { ref key } => {
                store_request!(
                    self,
                    message,
                    Ok::<kad::QueryId, DhtError>(
                        self.swarm.behaviour_mut().kad.get_record(Key::new(key))
                    )
                );
            }
            DhtRequest::PutRecord {
                ref key,
                ref value,
                quorum,
            } => {
                let value_owned = value.to_owned();
                if self.validate(value).await {
                    let record = Record {
                        key: Key::new(key),
                        value: value_owned,
                        publisher: None,
                        expires: None,
                    };
                    // Support a quorum of 0 when this is the only node in the
                    // network, in which case store it locally.
                    // Hopefully a temporary configuration in early bootstrapping.
                    if quorum == 0 {
                        let result = if let Err(_) =
                            self.swarm.behaviour_mut().kad.store_mut().put(record)
                        {
                            Err(DhtError::Error(String::from("Could not store record.")))
                        } else {
                            Ok(DhtResponse::PutRecord {
                                key: key.to_owned(),
                            })
                        };
                        message.respond(result);
                    } else {
                        let p2p_quorum = if quorum == 1 {
                            Quorum::One
                        } else {
                            Quorum::N(NonZeroUsize::new(quorum).unwrap())
                        };
                        store_request!(
                            self,
                            message,
                            self.swarm
                                .behaviour_mut()
                                .kad
                                .put_record(record, p2p_quorum)
                        );
                    }
                } else {
                    message.respond(Err(DhtError::ValidationError(value_owned)));
                }
            }
        };
    }

    /// Processes an incoming SwarmEvent, triggered from swarm activity or
    /// a swarm query. If a SwarmEvent has an associated DHTQuery,
    /// the pending query will be fulfilled.
    #[instrument(skip(self), level = "trace")]
    async fn process_swarm_event(&mut self, event: DHTSwarmEvent) {
        match event {
            SwarmEvent::Behaviour(DHTEvent::Kademlia(e)) => self.process_kad_event(e).await,
            SwarmEvent::Behaviour(DHTEvent::Identify(e)) => self.process_identify_event(e),
            SwarmEvent::Behaviour(DHTEvent::Void) => {}
            // The following events are currently handled only for debug logging.
            SwarmEvent::NewListenAddr {
                address: new_address,
                listener_id: new_listener_id,
            } => {
                let matches_pending = match (
                    self.active_listener.as_ref(),
                    self.pending_listener_request.as_ref(),
                ) {
                    (Some(active_listener), Some(_)) => &new_listener_id == active_listener,
                    _ => false,
                };

                if matches_pending {
                    let pending = self.pending_listener_request.take().unwrap();
                    let mut address = new_address;
                    address.push(Protocol::P2p(self.peer_id.into()));
                    pending.respond(Ok(DhtResponse::Address(address)));
                }
            }
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
            _ => {}
        }
    }

    async fn process_kad_event(&mut self, event: KademliaEvent) {
        match event {
            KademliaEvent::OutboundQueryProgressed { id, result, .. } => match result {
                QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(PeerRecord {
                    record: Record { key, value, .. },
                    ..
                }))) => {
                    if let Some(message) = self.requests.remove(&id) {
                        let is_valid = self.validate(&value).await;
                        // We don't want to propagate validation errors for all
                        // possible invalid records, but handle it similarly as if
                        // no record at all was found.
                        message.respond(Ok(DhtResponse::GetRecord(DhtRecord {
                            key: key.to_vec(),
                            value: if is_valid { Some(value) } else { None },
                        })));
                    };
                }
                QueryResult::GetRecord(Ok(kad::GetRecordOk::FinishedWithNoAdditionalRecord {
                    ..
                })) => {
                    if let Some(message) = self.requests.remove(&id) {
                        let key = {
                            if let DhtRequest::GetRecord { ref key, .. } = message.request {
                                key.to_owned()
                            } else {
                                panic!("Request must be GetRecord");
                            }
                        };
                        message.respond(Ok(DhtResponse::GetRecord(DhtRecord { key, value: None })));
                    }
                }
                QueryResult::GetRecord(Err(e)) => {
                    if let Some(message) = self.requests.remove(&id) {
                        match e {
                            kad::GetRecordError::NotFound { key, .. } => {
                                // Not finding a record is not an `Err` response,
                                // but simply a successful query with a `None` result.
                                message.respond(Ok(DhtResponse::GetRecord(DhtRecord {
                                    key: key.to_vec(),
                                    value: None,
                                })))
                            }
                            e => message.respond(Err(DhtError::from(e))),
                        };
                    }
                }
                QueryResult::PutRecord(Ok(kad::PutRecordOk { key })) => {
                    if let Some(message) = self.requests.remove(&id) {
                        message.respond(Ok(DhtResponse::PutRecord { key: key.to_vec() }));
                    }
                }
                QueryResult::PutRecord(Err(e)) => {
                    if let Some(message) = self.requests.remove(&id) {
                        message.respond(Err(DhtError::from(e)));
                    }
                }
                QueryResult::StartProviding(Ok(kad::AddProviderOk { .. })) => {
                    if let Some(message) = self.requests.remove(&id) {
                        message.respond(Ok(DhtResponse::Success));
                    }
                }
                QueryResult::StartProviding(Err(e)) => {
                    if let Some(message) = self.requests.remove(&id) {
                        message.respond(Err(DhtError::from(e)));
                    }
                }
                QueryResult::GetProviders(Ok(result)) => match result {
                    kad::GetProvidersOk::FoundProviders { providers, .. } => {
                        // Respond once we find any providers for now.
                        if !providers.is_empty() {
                            if let Some(message) = self.requests.remove(&id) {
                                message.respond(Ok(DhtResponse::GetProviders {
                                    providers: providers.into_iter().collect(),
                                }));
                            }
                        }
                    }
                    kad::GetProvidersOk::FinishedWithNoAdditionalRecord { .. } => {
                        // If this message has not been sent yet, then no providers
                        // have been discovered.
                        if let Some(message) = self.requests.remove(&id) {
                            message.respond(Ok(DhtResponse::GetProviders { providers: vec![] }));
                        }
                    }
                },
                QueryResult::GetProviders(Err(e)) => {
                    if let Some(message) = self.requests.remove(&id) {
                        message.respond(Err(DhtError::from(e)));
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
                        if self.validate(&rec.value).await {
                            if let Err(e) =
                                self.swarm.behaviour_mut().kad.store_mut().put(rec.clone())
                            {
                                warn!(
                                    "InboundRequest::PutRecord write failed: {:?} {:?}, {}",
                                    rec, source, e
                                );
                            }
                        } else {
                            warn!(
                                "InboundRequest::PutRecord validation failed: {:?} {:?}",
                                rec, source
                            );
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

    /// Starts listening on the provided address.
    fn listen(&mut self, address: &libp2p::Multiaddr) -> Result<(), DhtError> {
        self.stop_listening()?;
        let listener_id = self.swarm.listen_on(address.to_owned())?;
        self.active_listener = Some(listener_id);
        Ok(())
    }

    /// Stops listening on the provided address.
    fn stop_listening(&mut self) -> Result<(), DhtError> {
        if let Some(active_listener) = self.active_listener.take() {
            assert!(self.swarm.remove_listener(active_listener));
        }
        Ok(())
    }

    fn get_addresses(&mut self) -> Vec<Multiaddr> {
        self.swarm
            .listeners()
            .map(|addr| addr.to_owned())
            .collect::<Vec<Multiaddr>>()
    }

    fn get_external_addresses(&mut self) -> Vec<Multiaddr> {
        self.swarm
            .external_addresses()
            .map(|addr_record| addr_record.addr.to_owned())
            .collect::<Vec<Multiaddr>>()
    }

    /// Adds bootstrap peers to the routing table.
    async fn add_peers(&mut self, peers: &[libp2p::Multiaddr]) -> Result<(), DhtError> {
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
        Ok(())
    }

    fn execute_bootstrap(&mut self) -> Result<(), DhtError> {
        match self.swarm.behaviour_mut().kad.bootstrap() {
            Ok(_) => Ok(()),
            Err(_) => {
                // `NoKnownPeers` error is expected without any bootstrap peers.
                Ok(())
            }
        }
    }

    #[instrument(skip(self), level = "trace")]
    async fn validate(&mut self, data: &[u8]) -> bool {
        if let Some(v) = self.validator.as_mut() {
            v.validate(data).await
        } else {
            true
        }
    }
}

impl<V> fmt::Debug for DhtProcessor<V>
where
    V: Validator + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DHTNode")
            .field("peer_id", &self.peer_id)
            .field("config", &self.config)
            .finish()
    }
}
