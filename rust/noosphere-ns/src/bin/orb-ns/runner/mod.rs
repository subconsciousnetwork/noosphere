mod config;

use anyhow::Result;
pub use config::RunnerConfig;
use noosphere_ns::{Multiaddr, NameSystem, NameSystemClient};
use noosphere_storage::{MemoryStorage, SphereDb};
use std::net::TcpListener;
use std::sync::Arc;
use tokio::sync::Mutex;

#[cfg(feature = "api_server")]
mod inner {
    use super::*;
    pub use noosphere_ns::server::APIServer;

    pub fn serve(ns: Arc<Mutex<NameSystem>>, listener: TcpListener) -> APIServer {
        APIServer::serve(ns, listener)
    }
}
#[cfg(not(feature = "api_server"))]
mod inner {
    use super::*;
    pub struct APIServer {
        _ns: Arc<Mutex<NameSystem>>,
        _listener: TcpListener,
    }
    pub fn serve(ns: Arc<Mutex<NameSystem>>, listener: TcpListener) -> APIServer {
        APIServer {
            _ns: ns,
            _listener: listener,
        }
    }
}

use inner::*;

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

        // Request address from DHT to resolve default port (0) to
        // selected port.
        let address = node
            .listen(node_config.listening_address.to_owned())
            .await?;
        node.add_peers(node_config.peers.to_owned()).await?;

        println!("Listening on {}...", address);

        let wrapped_node = Arc::new(Mutex::new(node));

        let api_thread = if let Some(api_listener) = node_config.api_listener.take() {
            Some(serve(wrapped_node.clone(), api_listener))
        } else {
            None
        };

        name_systems.push(ActiveNameSystem {
            name_system: wrapped_node,
            _api_thread: api_thread,
        });
        addresses.push(address);
    }

    println!("Bootstrapping...");
    for (i, ns) in name_systems.iter_mut().enumerate() {
        let mut local_peers = addresses.clone();
        // Remove a node's own address from peers
        // TODO is this necessary?
        local_peers.remove(i);

        let node = ns.name_system.lock().await;

        if local_peers.len() != 0 {
            // Add both local peers (other nodes hosted in this process)
            // and provided bootstrap peers
            node.add_peers(local_peers).await?;
        }

        node.bootstrap().await?;
    }
    println!("Bootstrapped.");

    let mut tick = tokio::time::interval(tokio::time::Duration::from_secs(1));

    loop {
        tick.tick().await;
        let ns = name_systems.get(0).unwrap().name_system.lock().await;
        println!("network check  {:#?}", ns.network_info().await?);
    }

    Ok(())
}
