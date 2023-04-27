use crate::{DhtClient, DhtConfig, NameSystem};
use anyhow::Result;
use libp2p::Multiaddr;
use noosphere_core::authority::generate_ed25519_key;
use ucan::store::UcanJwtStore;

/// An in-process network of [NameSystem] nodes for testing.
pub struct NameSystemNetwork {
    nodes: Vec<NameSystem>,
    address: Multiaddr,
}

impl NameSystemNetwork {
    /// [NameSystem] nodes in the network.
    pub fn nodes(&self) -> &Vec<NameSystem> {
        &self.nodes
    }

    /// [NameSystem] nodes in the network.
    pub fn nodes_mut(&mut self) -> &mut Vec<NameSystem> {
        &mut self.nodes
    }

    /// Get reference to `index` [NameSystem] node.
    pub fn get(&self, index: usize) -> Option<&NameSystem> {
        self.nodes.get(index)
    }

    /// Get mutable reference to `index` [NameSystem] node.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut NameSystem> {
        self.nodes.get_mut(index)
    }

    /// An address of a node in the network to join.
    pub fn address(&self) -> &Multiaddr {
        &self.address
    }

    /// Generates a DHT network bootstrap node with `node_count`
    /// [NameSystem]s connected, each with a corresponding owner sphere.
    /// Useful for tests. All nodes share an underlying (cloned) store
    /// that may share state.
    pub async fn generate<S: UcanJwtStore + Clone + 'static>(
        node_count: usize,
        store: Option<S>,
    ) -> Result<Self> {
        let mut bootstrap_address: Option<Multiaddr> = None;
        let mut nodes = vec![];
        for _ in 0..node_count {
            let key = generate_ed25519_key();
            let node = NameSystem::new(&key, DhtConfig::default(), store.clone())?;
            let address = node.listen("/ip4/127.0.0.1/tcp/0".parse()?).await?;
            if let Some(address) = bootstrap_address.as_ref() {
                node.add_peers(vec![address.to_owned()]).await?;
            } else {
                bootstrap_address = Some(address);
            }
            nodes.push(node);
        }
        Ok(NameSystemNetwork {
            nodes,
            address: bootstrap_address.unwrap(),
        })
    }
}
