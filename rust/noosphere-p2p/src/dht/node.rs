use crate::dht::{
    behaviour::{DHTEvent, DHTSwarmEvent},
    errors::DHTError,
    swarm::{build_swarm, DHTSwarm},
    types::{DHTMessage, DHTMessageProcessor, DHTRequest, DHTResponse},
    DHTConfig,
};
use libp2p::futures::StreamExt;
use libp2p::kad;
use libp2p::{
    identify::IdentifyEvent,
    kad::{
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
use tracing;

/// The processing component of a [DHTNodeHandle]/[DHTNode] pair. Consumers
/// should only interface with a [DHTNode] via [DHTNodeHandle].
pub struct DHTNode {
    peer_id: PeerId,
    config: DHTConfig,
    processor: DHTMessageProcessor,
    swarm: DHTSwarm,
    //requests: Vec<OutstandingHandleRequest>,
    requests: HashMap<kad::QueryId, DHTMessage>,
    kad_last_range: Option<(Distance, Distance)>,
}

impl fmt::Debug for DHTNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DHTNode")
            .field("peer_id", &self.peer_id)
            .field("config", &self.config)
            .finish()
    }
}

macro_rules! dht_trace {
    ($self:expr, $event:expr) => {
        trace!("{:#?}::{:#?}", $self.peer_id.to_base58(), $event);
    };
}

// Temporary(?), exploring processing both requests that
// are bound by kad::QueryId, and requests that do not tie
// into DHT queries (like WaitForPeers).
macro_rules! store_request {
    /*
    ($self:expr, $message:expr) => {
        $self.requests.push(OutstandingHandleRequest {
            message: $message,
            query_id: None,
        });
    };*/
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

/*
macro_rules! take_requests {
    // Takes requests whose `message.request` property
    // matches provided pattern.
    ($self:expr, $match:pat) => {
        $self.requests.iter().filter(|r| match r.message.request {
            DHTRequest::WaitForPeers(4) => true,
            $match => true,
            _ => false,
        })
    };
    // Takes request whose `query_id` matches provided
    // QueryId.
    ($self:expr, $query_id:ident) => {
        $self
            .requests
            .iter()
            .find(|r| match r.query_id {
                Some(query_id) => query_id == $query_id,
                None => false,
            })
            .collect()
    };
}
*/

impl DHTNode {
    /// Creates a new DHTNode and begins processing in a new thread.
    pub fn spawn(
        config: DHTConfig,
        processor: DHTMessageProcessor,
    ) -> Result<tokio::task::JoinHandle<Result<(), DHTError>>, DHTError> {
        let peer_id = config.peer_id();
        let swarm = build_swarm(&peer_id, &config)?;
        let mut node = DHTNode {
            peer_id,
            config,
            processor,
            swarm,
            requests: HashMap::default(),
            kad_last_range: None,
        };

        Ok(tokio::spawn(async move { node.process().await }))
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

        let mut peer_dialing_tick =
            tokio::time::interval(Duration::from_secs(self.config.peer_dialing_interval));

        Ok(loop {
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
        })
    }

    /// Processes an incoming DHTMessage. Will attempt to respond
    /// immediately if possible (synchronous error or pulling value from cache),
    /// otherwise, the message will be mapped to a query, where it can be fulfilled
    /// later, most likely in `process_kad_result()`.
    fn process_message(&mut self, message: DHTMessage) {
        let span = trace_span!(
            "process_message",
            message = tracing::field::Empty,
            input = tracing::field::Empty,
            success = true
        )
        .entered();

        let behaviour = self.swarm.behaviour_mut();

        // Process client requests.
        match message.request {
            DHTRequest::GetProviders { ref name } => {
                span.record("message", "DHTRequest::GetProviders");
                span.record("input", format!("name={:?}", name));
                store_request!(
                    self,
                    message,
                    Ok::<kad::QueryId, DHTError>(behaviour.kad.get_providers(Key::new(name)))
                );
            }
            /*
            DHTRequest::WaitForPeers(peers) => {
                span.record("message", "DHTRequest::WaitForPeers");
                let info = self.swarm.network_info();
                if info.num_peers() >= peers {
                    message.respond(Ok(DHTResponse::Success));
                } else {
                    store_request!(self, message);
                }
            }
            */
            DHTRequest::Bootstrap => {
                message.respond(
                    self.execute_bootstrap()
                        .and_then(|_| Ok(DHTResponse::Success)),
                );
            }
            DHTRequest::GetNetworkInfo => {
                span.record("message", "DHTRequest::GetNetworkInfo");
                let info = self.swarm.network_info();
                message.respond(Ok(DHTResponse::GetNetworkInfo(info.into())));
            }
            DHTRequest::StartProviding { ref name } => {
                span.record("message", "DHTRequest::StartProviding");
                span.record("input", format!("name={:?}", name));
                store_request!(self, message, behaviour.kad.start_providing(Key::new(name)));
            }
            DHTRequest::GetRecord { ref name } => {
                span.record("message", "DHTRequest::GetRecord");
                span.record("input", format!("name={:?}", name));
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
                trace!("SetRecord");
                span.record("message", "DHTRequest::SetRecord");
                span.record("input", format!("name={:?}, value={:?}", name, value));
                let record = Record {
                    key: Key::new(name),
                    value: value.clone(),
                    publisher: None,
                    expires: None,
                };
                store_request!(self, message, behaviour.kad.put_record(record, Quorum::One));
            }
        };
    }

    /// Processes an incoming SwarmEvent, triggered from swarm activity or
    /// a swarm query. If a SwarmEvent has an associated DHTQuery,
    /// the pending query will be fulfilled.
    //fn process_swarm_event(&mut self, event: SwarmEvent<DHTEvent, std::io::Error>) {
    fn process_swarm_event(&mut self, event: DHTSwarmEvent) {
        dht_trace!(self, event);
        match event {
            SwarmEvent::Behaviour(DHTEvent::Kademlia(e)) => self.process_kad_event(e),
            SwarmEvent::Behaviour(DHTEvent::Identify(e)) => self.process_identify_event(e),
            // The following events are currently handled only for debug logging.
            SwarmEvent::NewListenAddr { address: _, .. } => {}
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                trace!("ConnectionEstablished: {:?}", peer_id);
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                trace!("ConnectionClosed: {:?}, {:?}", peer_id, cause);
            }
            SwarmEvent::IncomingConnection {
                local_addr,
                send_back_addr,
            } => {
                trace!(
                    "IncomingConnection: to {:?}, from {:?}",
                    local_addr,
                    send_back_addr
                );
            }
            SwarmEvent::IncomingConnectionError {
                local_addr,
                send_back_addr,
                error,
            } => {
                trace!(
                    "IncomingConnectionError: {:?} to {:?}, from {:?}",
                    error,
                    local_addr,
                    send_back_addr
                );
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error } => {
                trace!("OutgoingConnectionError: {:?} {:?}", error, peer_id);
            }
            SwarmEvent::BannedPeer { peer_id, .. } => {
                trace!("BannedPeer: {:?}", peer_id);
            }
            SwarmEvent::ExpiredListenAddr {
                listener_id,
                address,
            } => {
                trace!("ExpiredListenAddr: {:?}, {:?}", listener_id, address);
            }
            SwarmEvent::ListenerClosed {
                listener_id,
                addresses,
                reason,
            } => {
                trace!(
                    "ExpiredListenAddr: {:?}, {:?}, {:?}",
                    reason,
                    listener_id,
                    addresses
                );
            }
            SwarmEvent::ListenerError { listener_id, error } => {
                trace!("ListenerError: {:?}, {:?}", error, listener_id);
            }
            SwarmEvent::Dialing(peer_id) => {
                trace!("Dialing: {:?}", peer_id);
            }
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
                                value,
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
                    trace!(
                        "QueryResult::PutRecord Ok: {}",
                        String::from_utf8(key.to_vec()).unwrap()
                    );
                    if let Some(message) = self.requests.remove(&id) {
                        message.respond(Ok(DHTResponse::SetRecord { name: key.to_vec() }));
                    }
                }
                QueryResult::PutRecord(Err(e)) => {
                    trace!("QueryResult::PutRecord Err: {:#?}", e);
                    match e.clone() {
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
                    trace!(
                        "QueryResult::StartProviding Ok: {} ;;",
                        String::from_utf8(key.to_vec()).unwrap()
                    );
                    if let Some(message) = self.requests.remove(&id) {
                        message.respond(Ok(DHTResponse::StartProviding { name: key.to_vec() }));
                    }
                }
                QueryResult::StartProviding(Err(e)) => {
                    trace!("QueryResult::StartProviding Err: {} ;;", e.to_string());
                    if let Some(message) = self.requests.remove(&id) {
                        message.respond(Err(DHTError::from(e)));
                    }
                }
                QueryResult::GetProviders(Ok(kad::GetProvidersOk {
                    providers,
                    key,
                    closest_peers,
                })) => {
                    trace!(
                        "QueryResult::GetProviders OK: {:?} {:?} {:?};;",
                        providers,
                        key,
                        closest_peers
                    );
                    if let Some(message) = self.requests.remove(&id) {
                        message.respond(Ok(DHTResponse::GetProviders {
                            providers: providers.into_iter().collect(),
                            name: key.to_vec(),
                        }));
                    }
                }
                QueryResult::GetProviders(Err(e)) => {
                    trace!("QueryResult::GetProviders Err: {} ;;", e.to_string());
                    if let Some(message) = self.requests.remove(&id) {
                        message.respond(Err(DHTError::from(e)));
                    }
                }
                QueryResult::Bootstrap(Ok(kad::BootstrapOk {
                    peer,
                    num_remaining,
                })) => {
                    trace!("QueryResult::Bootstrap OK: {:?} {};;", peer, num_remaining);
                }
                QueryResult::Bootstrap(Err(kad::BootstrapError::Timeout {
                    peer,
                    num_remaining,
                })) => {
                    trace!(
                        "QueryResult::Bootstrap Err: {:?} {:?};;",
                        peer,
                        num_remaining
                    );
                }
                _ => {}
            },
            KademliaEvent::InboundRequest { request } => {
                let debug_str = match request {
                    kad::InboundRequest::FindNode { num_closer_peers } => {
                        format!("FindNode: closer peers {:?}", num_closer_peers)
                    }
                    kad::InboundRequest::GetProvider {
                        num_closer_peers,
                        num_provider_peers,
                    } => format!(
                        "GetProvider: provider peers {:?}, closer peers {:?}",
                        num_provider_peers, num_closer_peers
                    ),
                    kad::InboundRequest::AddProvider { record } => {
                        format!("AddProvider: {:?}", record)
                    }
                    kad::InboundRequest::GetRecord {
                        num_closer_peers,
                        present_locally,
                    } => format!(
                        "GetRecord: {:?} peers, local? {:?}",
                        num_closer_peers, present_locally
                    ),
                    kad::InboundRequest::PutRecord { source, record, .. } => {
                        match record {
                            None => warn!("InboundRequest::PutRecord failed; empty record"),
                            Some(rec) => {
                                if let Err(_) =
                                    self.swarm.behaviour_mut().kad.store_mut().put(rec.clone())
                                {
                                    warn!(
                                        "InboundRequest::PutRecord failed: {:?} {:?}",
                                        rec, source
                                    );
                                }
                            }
                        }
                        format!("PutRecord: {:?}", source)
                    }
                };
                trace!("InboundRequest::{:?}", debug_str);
            }
            KademliaEvent::RoutingUpdated {
                peer,
                is_new_peer,
                addresses,
                ..
            } => {
                if is_new_peer {
                    trace!("RoutingUpdated: (new peer) {:?}:{:?}", peer, addresses);
                } else {
                    trace!("RoutingUpdated: (old peer) {:?}:{:?}", peer, addresses);
                }
            }
            KademliaEvent::UnroutablePeer { peer } => {
                trace!("UnroutablePeer: {:?}", peer);
            }
            KademliaEvent::RoutablePeer { peer, address } => {
                trace!("RoutablePeer: {:?}:{:?}", peer, address);
            }
            KademliaEvent::PendingRoutablePeer { peer, address } => {
                trace!("PendingRoutablePeer : {:?}:{:?}", peer, address);
            }
        }
    }

    fn process_identify_event(&mut self, event: IdentifyEvent) {
        match event {
            IdentifyEvent::Received { peer_id, info } => {
                trace!(
                    "IdentifyEvent::Received: {:#?} {:#?}",
                    peer_id,
                    info.listen_addrs
                );
                if info
                    .protocols
                    .iter()
                    .any(|p| p.as_bytes() == kad::protocol::DEFAULT_PROTO_NAME)
                {
                    trace!("matching protocol!");
                    for addr in &info.listen_addrs {
                        trace!("adding addr {:#?}", addr);
                        self.swarm
                            .behaviour_mut()
                            .kad
                            .add_address(&peer_id, addr.clone());
                    }
                }
            }
            _ => {}
        }
    }

    /// Traverses the kbuckets to dial potential peers that
    /// are not yet connected. Implementation inspired by iroh:
    /// https://github.com/n0-computer/iroh/blob/main/iroh-p2p/src/node.rs
    fn dial_next_peer(&mut self) {
        trace!("dial next peer");
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
            debug!(
                "checking node {:?} in bucket range ({:?})",
                dial_opts.get_peer_id().unwrap(),
                range
            );

            if let Err(e) = self.swarm.dial(dial_opts) {
                warn!("failed to dial: {:?}", e);
            } else {
                warn!("success dial!");
            }
            self.kad_last_range = Some(range);
        }
    }

    fn start_listening(&mut self) -> Result<(), DHTError> {
        let addr = self.config.listening_address.clone();
        trace!("{}: start_listening() {}", self.peer_id, addr);
        self.swarm
            .listen_on(addr)
            .and_then(|_| Ok(()))
            .map_err(|e| DHTError::from(e))
    }

    fn execute_bootstrap(&mut self) -> Result<(), DHTError> {
        trace!("{}: execute_bootstrap()", self.peer_id);
        if self.config.bootstrap_peers.is_empty() {
            // Bootstrapping can't occur without bootstrap nodes
            return Ok(());
        }
        self.swarm
            .behaviour_mut()
            .kad
            .bootstrap()
            .and_then(|_| Ok(()))
            .map_err(|_| DHTError::NoKnownPeers)
    }
}

impl Drop for DHTNode {
    fn drop(&mut self) {
        //self.disconnect();
    }
}
