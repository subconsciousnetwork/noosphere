use crate::dht::behaviour::{DHTBehaviour, DHTEvent};
use crate::dht::types::DHTConfig;
use anyhow::Result;
use libp2p::kad;
use libp2p::kad::{Kademlia, KademliaConfig};
use libp2p::{swarm::SwarmBuilder, tokio_development_transport, Multiaddr, PeerId};
use std::{boxed::Box, future::Future, pin::Pin, str::FromStr, time::Duration};
use tokio;
const BOOTNODES: [&str; 4] = [
    "QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN",
    "QmQCU2EcMqAqQPR2i9bChDtGNJchTbq5TbXJJ16u19uLTa",
    "QmbLHAnMoJPWSCR5Zhtx6BHJX9KiKNN6tpvbUcqanj75Nb",
    "QmcZf59bWwK5XFi76CZX8cbJ4BhTzzA3gU1ZjYZcYW3dwt",
];

pub type DHTSwarm = libp2p::swarm::Swarm<DHTBehaviour>;

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

pub fn build_swarm(config: &DHTConfig) -> Result<DHTSwarm> {
    let local_peer_id = PeerId::from(config.keypair.public());

    // Set up a an encrypted DNS-enabled TCP Transport over the Mplex protocol
    // @TODO `tokio_development_transport` is not fit for production. Disect implementation
    // to determine what transports are appropriate.
    let transport = tokio_development_transport(config.keypair.clone())?;
    let mut behaviour = DHTBehaviour::new(&config, local_peer_id);

    // Add the bootnodes to the local routing table. `libp2p-dns` built
    // into the `transport` resolves the `dnsaddr` when Kademlia tries
    // to dial these nodes.
    let bootaddr = Multiaddr::from_str("/dnsaddr/bootstrap.libp2p.io")?;
    for peer in &BOOTNODES {
        behaviour
            .kad
            .add_address(&PeerId::from_str(peer)?, bootaddr.clone());
    }

    let handle = tokio::runtime::Handle::current();
    let executor_handle = Box::new(ExecutorHandle { handle });
    let swarm = SwarmBuilder::new(transport, behaviour, local_peer_id)
        .executor(executor_handle)
        .build();
    Ok(swarm)
}
