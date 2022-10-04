use crate::dht::types::{DHTConfig, DHTMessage, DHTMessageProcessor, DHTRequest, DHTResponse};
use anyhow::{anyhow, Result};
use libp2p::kad;
use libp2p::kad::{
    record, Kademlia, KademliaConfig, KademliaEvent, PeerRecord, QueryResult, Quorum, Record,
};
use libp2p::{
    futures::StreamExt,
    swarm::{SwarmBuilder, SwarmEvent},
    tokio_development_transport, Multiaddr, PeerId,
};
use std::{boxed::Box, future::Future, pin::Pin, str::FromStr, time::Duration};
use tokio;
const BOOTNODES: [&str; 4] = [
    "QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN",
    "QmQCU2EcMqAqQPR2i9bChDtGNJchTbq5TbXJJ16u19uLTa",
    "QmbLHAnMoJPWSCR5Zhtx6BHJX9KiKNN6tpvbUcqanj75Nb",
    "QmcZf59bWwK5XFi76CZX8cbJ4BhTzzA3gU1ZjYZcYW3dwt",
];

/// There's a bug in libp2p where the default executor is used
/// unless using [libp2p::swarm::SwarmBuilder] and setting a manual executor.
/// [ExecutorHandle] is used to wrap the underlying [tokio::runtime::Handle]
/// and pass into libp2p's SwarmBuilder.
/// https://github.com/libp2p/rust-libp2p/issues/2230
struct ExecutorHandle {
    handle: tokio::runtime::Handle,
}

impl libp2p::core::Executor for ExecutorHandle {
    fn exec(&self, future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) {
        self.handle.spawn(future);
    }
}

fn create_libp2p_swarm(config: &DHTConfig) -> Result<libp2p::swarm::Swarm<DHTBehaviour>> {
    let local_peer_id = PeerId::from(config.keypair.public());

    // Set up a an encrypted DNS-enabled TCP Transport over the Mplex protocol
    // @TODO `tokio_development_transport` is not fit for production. Disect implementation
    // to determine what transports are appropriate.
    let transport = tokio_development_transport(config.keypair.clone())?;

    // Create a swarm to manage peers and events.
    let mut cfg = KademliaConfig::default();
    cfg.set_query_timeout(Duration::from_secs(config.query_timeout.into()));
    // @TODO Use noosphere-fs instead of in-memory store.
    let store = kad::record::store::MemoryStore::new(local_peer_id);
    let mut behaviour = Kademlia::with_config(local_peer_id, store, cfg);

    // Add the bootnodes to the local routing table. `libp2p-dns` built
    // into the `transport` resolves the `dnsaddr` when Kademlia tries
    // to dial these nodes.
    let bootaddr = Multiaddr::from_str("/dnsaddr/bootstrap.libp2p.io")?;
    for peer in &BOOTNODES {
        behaviour.add_address(&PeerId::from_str(peer)?, bootaddr.clone());
    }

    let handle = tokio::runtime::Handle::current();
    let executor_handle = Box::new(ExecutorHandle { handle });
    let swarm = SwarmBuilder::new(transport, behaviour, local_peer_id)
        .executor(executor_handle)
        .build();
    Ok(swarm)
}

type DHTBehaviour = Kademlia<kad::record::store::MemoryStore>;

struct DHTPendingQuery {
    pub message: DHTMessage,
    pub query_id: kad::QueryId,
}

/// The processing component of a [DHTClient]/[DHTSwarm] pair. Consumers
/// should only interface with a [DHTSwarm] via [DHTClient].
pub struct DHTSwarm {
    config: DHTConfig,
    processor: DHTMessageProcessor,
}

impl DHTSwarm {
    /// Creates a new DHTSwarm and begins processing in a new thread.
    pub fn spawn(
        config: DHTConfig,
        processor: DHTMessageProcessor,
    ) -> tokio::task::JoinHandle<Result<()>> {
        let mut swarm = DHTSwarm { config, processor };
        tokio::spawn(async move { swarm.process().await })
    }

    /// Begin processing requests and connections on the DHT network
    /// in the current thread. Executes until the loop is broken, via
    /// either an unhandlable error or a terminate message (not yet implemented).
    async fn process(&mut self) -> Result<()> {
        let mut swarm = create_libp2p_swarm(&self.config)?;
        let mut pending_queries: Vec<DHTPendingQuery> = vec![];

        // swarm.behaviour_mut().get_closest_peers(to_search);
        Ok(loop {
            tokio::select! {
                message = self.processor.pull_message() => {
                    match message {
                        Some(m) => self.process_message(swarm.behaviour_mut(), &mut pending_queries, m),
                        // This occurs when sender is closed (client dropped).
                        // Exit the process loop for thread clean up.
                        None => break,
                    }
                }
                event = swarm.select_next_some() => {
                    self.process_swarm_event(swarm.behaviour_mut(), &mut pending_queries, event)
                }
            }
        })
    }

    /// Processes an incoming SwarmEvent, triggered from swarm activity or
    /// a swarm query via DHTMessage. If a SwarmEvent has an associated
    /// DHTPendingQuery, the pending query will be fulfilled.
    fn process_swarm_event(
        &mut self,
        _: &mut DHTBehaviour,
        pending_queries: &mut Vec<DHTPendingQuery>,
        event: SwarmEvent<KademliaEvent, std::io::Error>,
    ) {
        match event {
            SwarmEvent::Behaviour(KademliaEvent::OutboundQueryCompleted { id, result, .. }) => {
                match result {
                    QueryResult::GetRecord(Ok(ok)) => {
                        for PeerRecord {
                            record: Record { key, value, .. },
                            ..
                        } in ok.records
                        {
                            if let Some(pending_query) =
                                pop_pending_query_by_id(pending_queries, id)
                            {
                                pending_query.message.respond(Ok(DHTResponse::GetRecord {
                                    name: key.to_vec(),
                                    value,
                                }));
                            }
                        }
                    }
                    QueryResult::GetRecord(Err(e)) => {
                        if let Some(pending_query) = pop_pending_query_by_id(pending_queries, id) {
                            pending_query.message.respond(Err(anyhow!(e.to_string())));
                        }
                    }
                    QueryResult::PutRecord(Ok(kad::PutRecordOk { key })) => {
                        if let Some(pending_query) = pop_pending_query_by_id(pending_queries, id) {
                            pending_query
                                .message
                                .respond(Ok(DHTResponse::SetRecord { name: key.to_vec() }));
                        }
                    }
                    QueryResult::PutRecord(Err(e)) => {
                        if let Some(pending_query) = pop_pending_query_by_id(pending_queries, id) {
                            pending_query.message.respond(Err(anyhow!(e.to_string())));
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
                    QueryResult::StartProviding(Ok(kad::AddProviderOk { key })) => {
                        println!(
                            "Successfully put provider record {:?}",
                            std::str::from_utf8(key.as_ref()).unwrap()
                        );
                    }
                    QueryResult::StartProviding(Err(err)) => {
                        eprintln!("Failed to put provider record: {:?}", err);
                    }
                    */
                    _ => {}
                }
            }

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
            SwarmEvent::Behaviour(KademliaEvent::InboundRequest { request }) => {
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
            SwarmEvent::Behaviour(KademliaEvent::RoutingUpdated {
                peer,
                is_new_peer,
                addresses,
                ..
            }) => {
                if is_new_peer {
                    println!("RoutingUpdated: (new peer) {:?}:{:?}", peer, addresses);
                } else {
                    println!("RoutingUpdated: (old peer) {:?}:{:?}", peer, addresses);
                }
            }
            SwarmEvent::Behaviour(KademliaEvent::UnroutablePeer { peer }) => {
                println!("UnroutablePeer: {:?}", peer);
            }
            SwarmEvent::Behaviour(KademliaEvent::RoutablePeer { peer, address }) => {
                println!("RoutablePeer: {:?}:{:?}", peer, address);
            }
            SwarmEvent::Behaviour(KademliaEvent::PendingRoutablePeer { peer, address }) => {
                println!("PendingRoutablePeer : {:?}:{:?}", peer, address);
            }
        }
    }

    /// Processes an incoming DHTMessage. Will attempt to respond
    /// immediately if possible (synchronous error or pulling value from cache),
    /// otherwise, a DHTPendingQuery will be added to pending queries to handle
    /// after querying the swarm.
    fn process_message(
        &mut self,
        behaviour: &mut DHTBehaviour,
        pending_queries: &mut Vec<DHTPendingQuery>,
        message: DHTMessage,
    ) {
        // Process request. Result is `Ok(Some(query_id))` when a subsequent query
        // to the swarm needs to complete to resolve the request. Result is
        // `Ok(None)` when request can be processed immediately. Result is `Err()`
        // during synchronous failures.
        let result: Result<Option<kad::QueryId>> = match message.request {
            DHTRequest::GetRecord(ref key) => Ok(Some(
                behaviour.get_record(record::Key::new(key), Quorum::One),
            )),
            DHTRequest::SetRecord {
                ref name,
                ref value,
            } => {
                let record = Record {
                    key: record::Key::new(name),
                    value: value.clone(),
                    publisher: None,
                    expires: None,
                };
                behaviour
                    .put_record(record, Quorum::One)
                    .and_then(|q| Ok(Some(q)))
                    .map_err(|e| anyhow!(e.to_string()))
            }
        };

        match result {
            Ok(Some(query_id)) => {
                pending_queries.push(DHTPendingQuery { message, query_id });
            }
            Ok(None) => {}
            Err(e) => {
                message.respond(Err(anyhow!(e.to_string())));
            }
        }
    }
}

impl Drop for DHTSwarm {
    fn drop(&mut self) {
        //self.disconnect();
    }
}

fn pop_pending_query_by_id(
    pending_queries: &mut Vec<DHTPendingQuery>,
    id: libp2p::kad::QueryId,
) -> Option<DHTPendingQuery> {
    match pending_queries.iter().position(|p| p.query_id == id) {
        Some(index) => Some(pending_queries.remove(index)),
        None => None,
    }
}
