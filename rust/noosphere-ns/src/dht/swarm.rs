use crate::dht::{DhtConfig, DhtError};
use anyhow::anyhow;
use libp2p::{
    allow_block_list,
    identify::{Behaviour as Identify, Config as IdentifyConfig, Event as IdentifyEvent},
    identity::Keypair,
    kad::{
        self, Behaviour as KademliaBehaviour, Config as KademliaConfig, Event as KademliaEvent,
        Mode, StoreInserts as KademliaStoreInserts,
    },
    noise,
    swarm::{NetworkBehaviour, SwarmEvent},
    tls, yamux, PeerId, Swarm, SwarmBuilder,
};
use std::{result::Result, time::Duration};
use void::Void;

/// Protocols are responsible for determining how long
/// to keep idle connections alive.
const CONNECTION_TIMEOUT_SECONDS: u64 = 20;

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum DhtEvent {
    Kademlia(KademliaEvent),
    Identify(IdentifyEvent),
    Void,
}

impl From<KademliaEvent> for DhtEvent {
    fn from(event: KademliaEvent) -> Self {
        DhtEvent::Kademlia(event)
    }
}

impl From<IdentifyEvent> for DhtEvent {
    fn from(event: IdentifyEvent) -> Self {
        DhtEvent::Identify(event)
    }
}

impl From<Void> for DhtEvent {
    fn from(_: Void) -> Self {
        DhtEvent::Void
    }
}

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "DhtEvent", event_process = false)]
pub struct DhtBehavior {
    pub identify: Identify,
    pub kad: KademliaBehaviour<kad::store::MemoryStore>,
    blocked_peers: allow_block_list::Behaviour<allow_block_list::BlockedPeers>,
}

pub type DhtSwarmEvent = SwarmEvent<DhtEvent>;

impl DhtBehavior {
    pub fn new(keypair: &Keypair, local_peer_id: &PeerId, config: &DhtConfig) -> Self {
        let kad = {
            let mut cfg = KademliaConfig::default();
            cfg.set_query_timeout(Duration::from_secs(config.query_timeout.into()));
            // By default, all records from peers are automatically stored.
            // `FilterBoth` means it's the Kademlia behaviour handler's responsibility
            // to determine whether or not Provider records and KV records ("both") get stored,
            // where we implement logic to validate/prune incoming records.
            cfg.set_record_filtering(KademliaStoreInserts::FilterBoth);

            // These configurations only apply to Value records.
            cfg.set_record_ttl(Some(Duration::from_secs(config.record_ttl.into())));
            cfg.set_publication_interval(Some(Duration::from_secs(
                config.publication_interval.into(),
            )));
            cfg.set_replication_interval(Some(Duration::from_secs(
                config.replication_interval.into(),
            )));

            // These configurations are for Provider records. No replication interval available.
            cfg.set_provider_record_ttl(Some(Duration::from_secs(config.record_ttl.into())));
            cfg.set_provider_publication_interval(Some(Duration::from_secs(
                config.publication_interval.into(),
            )));

            // TODO(#99): Use SphereFS storage
            let store = kad::store::MemoryStore::new(local_peer_id.to_owned());
            let mut behaviour =
                KademliaBehaviour::with_config(local_peer_id.to_owned(), store, cfg);

            // TODO(#814): Updating to libp2p 0.53 introduced DHT nodes adjusting their modes dynamically,
            // where if a node does not have an external address, it switches to `Mode::Client`.
            // This improves the network by culling non-accessible nodes.
            // Manually setting to `Mode::Server` mode prevents this dynamic switching.
            // We could explore this optimization in the future, but for now,
            // we need to run on nightly rust, and some challenges with external addresses
            // in tests.
            // https://github.com/libp2p/rust-libp2p/pull/4503
            behaviour.set_mode(Some(Mode::Server));
            behaviour
        };

        let identify = {
            let config = IdentifyConfig::new("ipfs/1.0.0".into(), keypair.public())
                .with_agent_version(format!("noosphere-ns/{}", env!("CARGO_PKG_VERSION")));
            Identify::new(config)
        };

        DhtBehavior {
            kad,
            identify,
            blocked_peers: allow_block_list::Behaviour::default(),
        }
    }
}

/// Builds a configured [libp2p::swarm::Swarm] instance.
pub fn build_swarm(
    keypair: &Keypair,
    local_peer_id: &PeerId,
    config: &DhtConfig,
) -> Result<Swarm<DhtBehavior>, DhtError> {
    let swarm = SwarmBuilder::with_existing_identity(keypair.to_owned())
        .with_tokio()
        .with_tcp(
            Default::default(),
            (tls::Config::new, noise::Config::new),
            yamux::Config::default,
        )
        .map_err(|e| DhtError::from(anyhow!("{}", e)))?
        .with_dns()
        .map_err(|e| DhtError::from(anyhow!("{}", e)))?
        .with_behaviour(|keypair| DhtBehavior::new(keypair, local_peer_id, config))
        .map_err(|e| DhtError::from(anyhow!("{}", e)))?
        .with_swarm_config(|mut cfg| {
            cfg = cfg.with_idle_connection_timeout(std::time::Duration::from_secs(
                CONNECTION_TIMEOUT_SECONDS,
            ));
            cfg
        })
        .build();
    Ok(swarm)
}
