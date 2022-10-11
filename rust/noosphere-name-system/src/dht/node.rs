use crate::dht::{
    behaviour::DHTEvent,
    swarm::{build_swarm, DHTSwarm},
    types::{DHTMessage, DHTMessageProcessor, DHTRequest, DHTResponse},
    utils, DHTConfig,
};
use anyhow::{anyhow, Result};
use libp2p::futures::StreamExt;
use libp2p::kad;
use libp2p::kad::{record::Key, KademliaEvent, PeerRecord, QueryResult, Quorum, Record};
use libp2p::{swarm::SwarmEvent, Multiaddr, PeerId};
use std::collections::HashMap;
use std::fmt;
use tokio;

/// The processing component of a [DHTClient]/[DHTNode] pair. Consumers
/// should only interface with a [DHTNode] via [DHTClient].
pub struct DHTNode {
    peer_id: PeerId,
    config: DHTConfig,
    processor: DHTMessageProcessor,
    swarm: DHTSwarm,
    queries: HashMap<kad::QueryId, DHTMessage>,
}

impl fmt::Debug for DHTNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DHTNode")
            .field("peer_id", &self.peer_id)
            .field("config", &self.config)
            .finish()
    }
}

impl DHTNode {
    /// Creates a new DHTNode and begins processing in a new thread.
    pub fn spawn(
        config: DHTConfig,
        processor: DHTMessageProcessor,
    ) -> Result<tokio::task::JoinHandle<Result<()>>> {
        let peer_id = utils::peer_id_from_key_with_sha256(&config.keypair.public())?;
        let swarm = build_swarm(&peer_id, &config)?;
        let mut node = DHTNode {
            peer_id,
            config,
            processor,
            swarm,
            queries: HashMap::new(),
        };

        debug!("Spawning DHTNode {:#?}", node);
        Ok(tokio::spawn(async move { node.process().await }))
    }

    /// Begin processing requests and connections on the DHT network
    /// in the current thread. Executes until the loop is broken, via
    /// either an unhandlable error or a terminate message (not yet implemented).
    async fn process(&mut self) -> Result<()> {
        //self.swarm.listen_on("/ip4/127.0.0.1".parse().unwrap())?;

        Ok(loop {
            tokio::select! {
                message = self.processor.pull_message() => {
                    match message {
                        Some(m) => self.process_message(m),
                        // This occurs when sender is closed (client dropped).
                        // Exit the process loop for thread clean up.
                        None => break,
                    }
                }
                event = self.swarm.select_next_some() => {
                    self.process_swarm_event(event)
                }
            }
        })
    }

    /// Processes an incoming DHTMessage. Will attempt to respond
    /// immediately if possible (synchronous error or pulling value from cache),
    /// otherwise, a DHTQuery will be added to pending queries to handle
    /// after querying the swarm.
    fn process_message(&mut self, message: DHTMessage) {
        let behaviour = self.swarm.behaviour_mut();
        // Process request. Result is `Ok(Some(query_id))` when a subsequent query
        // to the swarm needs to complete to resolve the request. Result is
        // `Ok(None)` when request can be processed immediately. Result is `Err()`
        // during synchronous failures.
        let result: Result<Option<kad::QueryId>> = match message.request {
            DHTRequest::StartProviding { ref name } => {
                match behaviour.kad.start_providing(Key::new(name)) {
                    Ok(query_id) => Ok(Some(query_id)),
                    Err(e) => Err(anyhow!(e.to_string())),
                }
            }
            DHTRequest::GetRecord { ref name } => {
                Ok(Some(behaviour.kad.get_record(Key::new(name), Quorum::One)))
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
                behaviour
                    .kad
                    .put_record(record, Quorum::One)
                    .and_then(|q| Ok(Some(q)))
                    .map_err(|e| anyhow!(e.to_string()))
            }
        };

        match result {
            Ok(Some(query_id)) => {
                self.queries.insert(query_id, message);
            }
            Ok(None) => {}
            Err(e) => {
                message.respond(Err(anyhow!(e.to_string())));
            }
        }
    }

    /// Processes an incoming SwarmEvent, triggered from swarm activity or
    /// a swarm query. If a SwarmEvent has an associated DHTQuery,
    /// the pending query will be fulfilled.
    fn process_swarm_event(&mut self, event: SwarmEvent<DHTEvent, std::io::Error>) {
        match event {
            SwarmEvent::Behaviour(dht_event) => match dht_event {
                DHTEvent::Kademlia(e) => self.process_kad_event(e),
            },
            // The following events are currently handled only for debug logging.
            SwarmEvent::NewListenAddr { address, .. } => {
                println!("NewListenAddr: {:?}", address);
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                println!("ConnectionEstablished: {:?}", peer_id);
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                println!("ConnectionClosed: {:?}, {:?}", peer_id, cause);
            }
            SwarmEvent::IncomingConnection {
                local_addr,
                send_back_addr,
            } => {
                println!(
                    "IncomingConnection: to {:?}, from {:?}",
                    local_addr, send_back_addr
                );
            }
            SwarmEvent::IncomingConnectionError {
                local_addr,
                send_back_addr,
                error,
            } => {
                println!(
                    "IncomingConnectionError: {:?} to {:?}, from {:?}",
                    error, local_addr, send_back_addr
                );
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error } => {
                println!("OutgoingConnectionError: {:?} {:?}", error, peer_id);
            }
            SwarmEvent::BannedPeer { peer_id, .. } => {
                println!("BannedPeer: {:?}", peer_id);
            }
            SwarmEvent::ExpiredListenAddr {
                listener_id,
                address,
            } => {
                println!("ExpiredListenAddr: {:?}, {:?}", listener_id, address);
            }
            SwarmEvent::ListenerClosed {
                listener_id,
                addresses,
                reason,
            } => {
                println!(
                    "ExpiredListenAddr: {:?}, {:?}, {:?}",
                    reason, listener_id, addresses
                );
            }
            SwarmEvent::ListenerError { listener_id, error } => {
                println!("ListenerError: {:?}, {:?}", error, listener_id);
            }
            SwarmEvent::Dialing(peer_id) => {
                println!("Dialing: {:?}", peer_id);
            }
        }
    }

    fn process_kad_event(&mut self, event: KademliaEvent) {
        match event {
            KademliaEvent::OutboundQueryCompleted { id, result, .. } => {
                match result {
                    QueryResult::GetRecord(Ok(ok)) => {
                        for PeerRecord {
                            record: Record { key, value, .. },
                            ..
                        } in ok.records
                        {
                            if let Some(message) = self.queries.remove(&id) {
                                message.respond(Ok(DHTResponse::GetRecord {
                                    name: key.to_vec(),
                                    value,
                                }));
                            }
                        }
                    }
                    QueryResult::GetRecord(Err(e)) => {
                        if let Some(message) = self.queries.remove(&id) {
                            message.respond(Err(anyhow!(e.to_string())));
                        }
                    }
                    QueryResult::PutRecord(Ok(kad::PutRecordOk { key })) => {
                        if let Some(message) = self.queries.remove(&id) {
                            message.respond(Ok(DHTResponse::SetRecord { name: key.to_vec() }));
                        }
                    }
                    QueryResult::PutRecord(Err(e)) => {
                        if let Some(message) = self.queries.remove(&id) {
                            message.respond(Err(anyhow!(e.to_string())));
                        }
                    }
                    QueryResult::StartProviding(Ok(kad::AddProviderOk { key })) => {
                        if let Some(message) = self.queries.remove(&id) {
                            message.respond(Ok(DHTResponse::StartProviding { name: key.to_vec() }));
                        }
                    }
                    QueryResult::StartProviding(Err(e)) => {
                        if let Some(message) = self.queries.remove(&id) {
                            message.respond(Err(anyhow!(e.to_string())));
                        }
                    }
                    /*
                    QueryResult::GetProviders(Ok(ok)) => {
                        for peer in ok.providers {
                            println!(
                                "Peer {:?} provides key {:?}",
                                peer,
                                std::str::from_utf8(ok.key.as_ref()).unwrap()
                            );
                        }
                    }
                    QueryResult::GetProviders(Err(err)) => {
                        eprintln!("Failed to get providers: {:?}", err);
                    }
                    */
                    _ => {}
                }
            }
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
                        format!("PutRecord: {:?} {:?}", source, record)
                    }
                };
                println!("InboundRequest::{:?}", debug_str);
            }
            KademliaEvent::RoutingUpdated {
                peer,
                is_new_peer,
                addresses,
                ..
            } => {
                if is_new_peer {
                    println!("RoutingUpdated: (new peer) {:?}:{:?}", peer, addresses);
                } else {
                    println!("RoutingUpdated: (old peer) {:?}:{:?}", peer, addresses);
                }
            }
            KademliaEvent::UnroutablePeer { peer } => {
                println!("UnroutablePeer: {:?}", peer);
            }
            KademliaEvent::RoutablePeer { peer, address } => {
                println!("RoutablePeer: {:?}:{:?}", peer, address);
            }
            KademliaEvent::PendingRoutablePeer { peer, address } => {
                println!("PendingRoutablePeer : {:?}:{:?}", peer, address);
            }
        }
    }
}

impl Drop for DHTNode {
    fn drop(&mut self) {
        //self.disconnect();
    }
}
