use crate::dht::behaviour::DHTBehaviour;
use crate::dht::DHTConfig;
use anyhow::Result;
use libp2p::{swarm::SwarmBuilder, tokio_development_transport, PeerId};
use std::{boxed::Box, future::Future, pin::Pin};
use tokio;

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

pub fn build_swarm(local_peer_id: &PeerId, config: &DHTConfig) -> Result<DHTSwarm> {
    // Set up a an encrypted DNS-enabled TCP Transport over the Mplex protocol
    // @TODO `tokio_development_transport` is not fit for production. Disect implementation
    // to determine what transports are appropriate.
    let transport = tokio_development_transport(config.keypair.clone())?;
    let behaviour = DHTBehaviour::new(&config, local_peer_id.to_owned())?;

    let handle = tokio::runtime::Handle::current();
    let executor_handle = Box::new(ExecutorHandle { handle });
    let swarm = SwarmBuilder::new(transport, behaviour, local_peer_id.to_owned())
        .executor(executor_handle)
        .build();
    Ok(swarm)
}
