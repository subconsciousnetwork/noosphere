mod config;

use anyhow::Result;
pub use config::RunnerConfig;
use futures::stream::empty;
use futures::StreamExt;
use noosphere_ns::{Multiaddr, NameSystem, NameSystemClient};
use noosphere_storage::{MemoryStorage, SphereDb};
use std::{net::TcpListener, sync::Arc};
use tokio::sync::Mutex;
use tracing::*;

#[cfg(feature = "api-server")]
use noosphere_ns::server::APIServer;

#[cfg(not(feature = "api-server"))]
struct APIServer;
#[cfg(not(feature = "api-server"))]
impl APIServer {
    pub fn serve(_ns: Arc<Mutex<NameSystem>>, _listener: TcpListener) -> Self {
        APIServer {}
    }
}

struct ActiveNameSystem {
    name_system: Arc<Mutex<NameSystem>>,
    _api_thread: Option<APIServer>,
}

/// Runner runs one or many DHT nodes based off of provided
/// configuration from a [CLICommand].
pub async fn run(mut config: RunnerConfig) -> Result<()> {
    let mut name_systems: Vec<ActiveNameSystem> = vec![];
    let mut addresses: Vec<Multiaddr> = vec![];

    for node_config in config.nodes.iter_mut() {
        let store = SphereDb::new(&MemoryStorage::default()).await?;
        let node = NameSystem::new(
            &node_config.key_material,
            store,
            node_config.dht_config.to_owned(),
        )?;

        if let Some(listening_address) = node_config.listening_address.take() {
            // Request address from DHT to resolve default port (0) to
            // selected port.
            let address = node.listen(listening_address.to_owned()).await?;
            info!("Listening on {}...", address);
            addresses.push(address);
        }

        node.add_peers(node_config.peers.to_owned()).await?;

        let wrapped_node = Arc::new(Mutex::new(node));

        let api_thread = if cfg!(feature = "api-server") {
            if let Some(api_address) = node_config.api_address.take() {
                let api_listener = TcpListener::bind(api_address)?;
                info!(
                    "Operator API listening on {}...",
                    api_listener.local_addr()?
                );
                Some(APIServer::serve(wrapped_node.clone(), api_listener))
            } else {
                None
            }
        } else {
            None
        };

        name_systems.push(ActiveNameSystem {
            name_system: wrapped_node,
            _api_thread: api_thread,
        });
    }

    info!("Bootstrapping...");
    for (_i, ns) in name_systems.iter_mut().enumerate() {
        let node = ns.name_system.lock().await;

        if !addresses.is_empty() {
            // Add both local peers (other nodes hosted in this process)
            // and provided bootstrap peers
            node.add_peers(addresses.clone()).await?;
        }

        node.bootstrap().await?;
    }
    info!("Bootstrapped.");

    // Construct an empty stream to keep this runner function alive.
    // This is where we could add streaming events from the DHT.
    let mut empty_stream = empty::<()>();
    while let None = empty_stream.next().await {}

    Ok(())
}
