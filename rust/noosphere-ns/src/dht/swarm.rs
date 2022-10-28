use crate::dht::errors::DHTError;
use crate::dht::DHTConfig;
use libp2p::{
    core::muxing::StreamMuxerBox,
    core::transport::Boxed,
    core::upgrade,
    dns,
    identify::{Behaviour as Identify, Config as IdentifyConfig, Event as IdentifyEvent},
    identity::Keypair,
    kad::{self, Kademlia, KademliaConfig, KademliaEvent, KademliaStoreInserts},
    mplex, noise,
    swarm::SwarmBuilder,
    swarm::{self, ConnectionHandler, IntoConnectionHandler, SwarmEvent},
    tcp, yamux, NetworkBehaviour, PeerId, Transport,
};
use std::time::Duration;
use std::{boxed::Box, future::Future, pin::Pin};
use std::{io, result::Result};
use tokio;

#[derive(Debug)]
pub enum DHTEvent {
    Kademlia(KademliaEvent),
    Identify(IdentifyEvent),
}

impl From<KademliaEvent> for DHTEvent {
    fn from(event: KademliaEvent) -> Self {
        DHTEvent::Kademlia(event)
    }
}

impl From<IdentifyEvent> for DHTEvent {
    fn from(event: IdentifyEvent) -> Self {
        DHTEvent::Identify(event)
    }
}

pub type DHTSwarmEvent = SwarmEvent<
            <DHTBehaviour as swarm::NetworkBehaviour>::OutEvent,
            <<<DHTBehaviour as swarm::NetworkBehaviour>::ConnectionHandler as IntoConnectionHandler>::Handler as ConnectionHandler>::Error>;

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "DHTEvent", event_process = false)]
pub struct DHTBehaviour {
    pub identify: Identify,
    pub kad: Kademlia<kad::record::store::MemoryStore>,
}

impl DHTBehaviour {
    pub fn new(keypair: &Keypair, local_peer_id: &PeerId, config: &DHTConfig) -> Self {
        let kad = {
            let mut cfg = KademliaConfig::default();
            cfg.set_query_timeout(Duration::from_secs(config.query_timeout.into()));
            cfg.set_record_filtering(KademliaStoreInserts::FilterBoth);

            // TODO(#99): Use SphereFS storage
            let store = kad::record::store::MemoryStore::new(local_peer_id.to_owned());
            Kademlia::with_config(local_peer_id.to_owned(), store, cfg)
        };

        let identify = {
            let config = IdentifyConfig::new("ipfs/1.0.0".into(), keypair.public())
                .with_agent_version(format!("noosphere-ns/{}", env!("CARGO_PKG_VERSION")));
            Identify::new(config)
        };

        DHTBehaviour { kad, identify }
    }
}

pub type DHTSwarm = libp2p::swarm::Swarm<DHTBehaviour>;

/// Creates the Transport mechanism that describes how peers communicate.
/// Currently, mostly an inlined form of `libp2p::tokio_development_transport`.
fn build_transport(keypair: &Keypair) -> Result<Boxed<(PeerId, StreamMuxerBox)>, io::Error> {
    let transport = dns::TokioDnsConfig::system(tcp::TokioTcpTransport::new(
        tcp::GenTcpConfig::new().nodelay(true),
    ))?;

    let noise_keys = noise::Keypair::<noise::X25519Spec>::new()
        .into_authentic(keypair)
        .expect("Noise key generation failed.");

    Ok(transport
        .upgrade(upgrade::Version::V1)
        .authenticate(noise::NoiseConfig::xx(noise_keys).into_authenticated())
        .multiplex(upgrade::SelectUpgrade::new(
            yamux::YamuxConfig::default(),
            mplex::MplexConfig::default(),
        ))
        .timeout(std::time::Duration::from_secs(20))
        .boxed())
}

/// Builds a configured [libp2p::swarm::Swarm] instance.
pub fn build_swarm(
    keypair: &Keypair,
    local_peer_id: &PeerId,
    config: &DHTConfig,
) -> Result<DHTSwarm, DHTError> {
    struct ExecutorHandle {
        handle: tokio::runtime::Handle,
    }

    impl libp2p::core::Executor for ExecutorHandle {
        fn exec(&self, future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) {
            self.handle.spawn(future);
        }
    }

    let transport = build_transport(keypair).map_err(DHTError::from)?;
    let behaviour = DHTBehaviour::new(keypair, local_peer_id, config);

    let handle = tokio::runtime::Handle::current();
    let executor_handle = Box::new(ExecutorHandle { handle });
    let swarm = SwarmBuilder::new(transport, behaviour, local_peer_id.to_owned())
        .executor(executor_handle)
        .build();
    Ok(swarm)
}
