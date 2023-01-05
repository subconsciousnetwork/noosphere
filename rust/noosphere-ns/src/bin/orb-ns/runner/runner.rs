use crate::runner::RunnerConfig;
use anyhow::Result;
use noosphere_ns::{Multiaddr, NameSystem};
use noosphere_storage::{MemoryStorage, SphereDb};

/// Runner runs one or many DHT nodes based off of provided
/// configuration from a [CLICommand].
pub struct Runner {
    config: RunnerConfig,
    name_systems: Option<Vec<NameSystem>>,
}

impl Runner {
    pub fn new(config: RunnerConfig) -> Self {
        Runner {
            config,
            name_systems: None,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut nodes: Vec<NameSystem> = vec![];
        let mut addresses: Vec<Multiaddr> = vec![];

        for node_config in self.config.nodes.iter() {
            let store = SphereDb::new(&MemoryStorage::default()).await?;
            let mut node = NameSystem::new(
                &node_config.key_material,
                store,
                node_config.dht_config.to_owned(),
            )?;
            node.start_listening(node_config.listening_address.to_owned())
                .await?;
            node.add_peers(node_config.peers.to_owned()).await?;

            // Request address from DHT to resolve default port (0) to
            // selected port.
            let p2p_addresses = node.p2p_addresses().await?;
            let p2p_address = p2p_addresses.first().unwrap().to_owned();

            println!("Listening on {}...", p2p_address);

            nodes.push(node);
            addresses.push(p2p_address);
        }

        println!("Bootstrapping...");
        for (i, node) in nodes.iter_mut().enumerate() {
            let mut local_peers = addresses.clone();
            // Remove a node's own address from peers
            // TODO is this necessary?
            local_peers.remove(i);
            // Add both local peers (other nodes hosted in this process)
            // and provided bootstrap peers
            node.add_peers(local_peers).await?;
            node.bootstrap().await?;
        }
        println!("Bootstrapped.");
        self.name_systems = Some(nodes);
        Ok(())
    }
}

impl From<RunnerConfig> for Runner {
    fn from(config: RunnerConfig) -> Self {
        Runner::new(config)
    }
}
