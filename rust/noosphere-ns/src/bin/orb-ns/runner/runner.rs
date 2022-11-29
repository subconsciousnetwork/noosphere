use crate::runner::RunnerConfig;
use crate::utils;
use anyhow::Result;
use noosphere_ns::{
    dht::{DHTConfig, DHTNode},
    Validator, BOOTSTRAP_PEERS,
};
use noosphere_storage::{SphereDb, MemoryStorage};

type NSNode = DHTNode<Validator<MemoryStorage>>;

/// Runner runs one or many DHT nodes based off of provided
/// configuration from a [CLICommand].
pub struct Runner {
    config: RunnerConfig,
    dht_nodes: Vec<NSNode>,
}

impl Runner {
    pub fn new(config: RunnerConfig) -> Self {
        Runner {
            config,
            dht_nodes: vec![],
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        let store = SphereDb::new(&MemoryStorage::default()).await?;

        for node_config in self.config.nodes.iter() {
            let config = DHTConfig {
                listening_address: Some(node_config.listening_address.clone()),
                ..Default::default()
            };
            let mut node = DHTNode::new(
                &node_config.key_material,
                None,
                Validator::new(&store),
                &config,
            )?;
            let listening_address = node.p2p_address().expect("has address").to_owned();
            let bootstrap_peers =
                utils::filter_bootstrap_peers(&listening_address, &BOOTSTRAP_PEERS[..]);

            println!("Using bootstrap peers at {:#?}", &bootstrap_peers);
            node.add_peers(&bootstrap_peers)?;
            node.run()?;
            println!("Listening on {}...", listening_address);

            println!("Bootstrapping...");
            node.bootstrap().await?;
            println!("Bootstrapped.");
            self.dht_nodes.push(node);
        }
        Ok(())
    }
}

impl From<RunnerConfig> for Runner {
    fn from(config: RunnerConfig) -> Self {
        Runner::new(config)
    }
}
