mod behaviour;
mod channel;
mod config;
mod errors;
mod handle;
mod node;
mod swarm;
mod transport;
mod types;

pub use config::DHTConfig;
pub use errors::DHTError;
pub use handle::DHTNodeHandle;
pub use types::{DHTNetworkInfo, DHTStatus};

/// Spawns a networking thread with a DHT node, returning the
/// corresponding [DHTNodeHandle].
pub fn spawn_dht_node(config: DHTConfig) -> Result<DHTNodeHandle, DHTError> {
    DHTNodeHandle::new(config)
}
