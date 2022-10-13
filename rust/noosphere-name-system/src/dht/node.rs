use crate::dht::{
    behaviour::DHTEvent,
    errors::DHTError,
    swarm::{build_swarm, DHTSwarm},
    types::{DHTMessage, DHTMessageProcessor, DHTRequest, DHTResponse},
    DHTConfig,
};
use anyhow::anyhow;
use libp2p::futures::StreamExt;
use libp2p::kad;
use libp2p::kad::{
    record,
    record::{store::RecordStore, Key},
    KademliaEvent, PeerRecord, QueryResult, Quorum, Record,
};
use libp2p::{swarm::SwarmEvent, PeerId};
use std::fmt;
use std::{collections::HashMap, time::Duration};
use tokio;
use tracing;

/// The processing component of a [DHTClient]/[DHTNode] pair. Consumers
/// should only interface with a [DHTNode] via [DHTClient].
pub struct DHTNode {
    peer_id: PeerId,
    config: DHTConfig,
    processor: DHTMessageProcessor,
    swarm: DHTSwarm,
    requests: HashMap<kad::QueryId, DHTMessage>,
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

macro_rules! dht_map_request {
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
            requests: HashMap::new(),
        };

        debug!("Spawning DHTNode {:#?}", node);
        Ok(tokio::spawn(async move { node.process().await }))
    }

    /// Begin processing requests and connections on the DHT network
    /// in the current thread. Executes until the loop is broken, via
    /// either an unhandlable error or a terminate message (not yet implemented).
    async fn process(&mut self) -> Result<(), DHTError> {
        if let Some(address) = self.config.listening_address.as_ref() {
            trace!("Listening on {}", address);
            if let Err(e) = self.swarm.listen_on(address.to_owned()) {
                error!("Listening Error! {:#?}", e);
                return Err(DHTError::Error(format!(
                    "Failed to listen on address {:#?}",
                    address
                )));
            }
        }

        let mut bootstrap_tick =
            tokio::time::interval(Duration::from_secs(self.config.bootstrap_interval));

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
                _ = bootstrap_tick.tick() => {
                    self.execute_bootstrap();
                }
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
                dht_map_request!(self, message, behaviour.kad.start_providing(Key::new(name)));
            }
            DHTRequest::GetRecord { ref name } => {
                span.record("message", "DHTRequest::GetRecord");
                span.record("input", format!("name={:?}", name));
                dht_map_request!(
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
                dht_map_request!(self, message, behaviour.kad.put_record(record, Quorum::One));
            }
        };
    }

    /// Processes an incoming SwarmEvent, triggered from swarm activity or
    /// a swarm query. If a SwarmEvent has an associated DHTQuery,
    /// the pending query will be fulfilled.
    fn process_swarm_event(&mut self, event: SwarmEvent<DHTEvent, std::io::Error>) {
        dht_trace!(self, event);
        match event {
            SwarmEvent::Behaviour(dht_event) => match dht_event {
                DHTEvent::Kademlia(e) => self.process_kad_event(e),
            },
            // The following events are currently handled only for debug logging.
            SwarmEvent::NewListenAddr { address, .. } => {}
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
                }
                QueryResult::GetProviders(Err(e)) => {
                    trace!("QueryResult::GetProviders Err: {} ;;", e.to_string());
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
                                if let Err(e) =
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

    fn execute_bootstrap(&mut self) -> Result<(), DHTError> {
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
