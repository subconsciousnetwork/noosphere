#![cfg(test)]
use futures::future::try_join_all;
use libp2p::{self, Multiaddr};
use noosphere_core::authority::generate_ed25519_key;
use noosphere_ns::dht::{DhtConfig, DhtError, DhtNode, RecordValidator};
use noosphere_ns::helpers::generate_default_listening_address;
use std::future::Future;
use std::time::Duration;

pub async fn wait_ms(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

async fn await_or_timeout<T>(
    timeout_ms: u64,
    future: impl Future<Output = T>,
    message: String,
) -> T {
    tokio::select! {
        _ = wait_ms(timeout_ms) => { panic!("timed out: {}", message); }
        result = future => { result }
    }
}

pub async fn swarm_command<'a, TFuture, F, T, E>(
    nodes: &'a mut [DhtNode],
    func: F,
) -> Result<Vec<T>, E>
where
    F: FnMut(&'a mut DhtNode) -> TFuture,
    TFuture: Future<Output = Result<T, E>>,
{
    let futures: Vec<_> = nodes.iter_mut().map(func).collect();
    try_join_all(futures).await
}

pub async fn create_nodes_with_peers<V: RecordValidator + Clone + 'static>(
    client_count: usize,
    bootstrap_addresses: &[Multiaddr],
    validator: Option<V>,
) -> Result<Vec<DhtNode>, DhtError> {
    let mut client_nodes: Vec<DhtNode> = vec![];
    for _ in 0..client_count {
        let key_material = generate_ed25519_key();
        let config = DhtConfig::default();
        let node = DhtNode::new(&key_material, config, validator.clone())?;
        node.add_peers(bootstrap_addresses.to_vec()).await?;
        node.listen(generate_default_listening_address()).await?;
        client_nodes.push(node);
    }
    Ok(client_nodes)
}

/// Creates `count` bootstrap nodes, each node using all other
/// bootstrap nodes as bootstrap peers.
pub async fn create_bootstrap_nodes<V: RecordValidator + Clone + 'static>(
    count: usize,
    validator: Option<V>,
) -> Result<(Vec<DhtNode>, Vec<Multiaddr>), DhtError> {
    let mut nodes: Vec<DhtNode> = vec![];
    let mut addresses: Vec<Multiaddr> = vec![];
    for _ in 0..count {
        let key_material = generate_ed25519_key();
        let config = DhtConfig::default();
        let node = DhtNode::new(&key_material, config, validator.clone())?;
        let address = node.listen(generate_default_listening_address()).await?;
        addresses.push(address);
        nodes.push(node);
    }

    for (i, node) in nodes.iter_mut().enumerate() {
        let mut peers = addresses.clone();
        // Remove a node's own address from peers
        peers.remove(i);
        node.add_peers(peers).await?;
    }
    Ok((nodes, addresses))
}

pub async fn initialize_network<V: RecordValidator + Clone + 'static>(
    bootstrap_count: usize,
    client_count: usize,
    validator: Option<V>,
) -> Result<(Vec<DhtNode>, Vec<DhtNode>, Vec<Multiaddr>), DhtError> {
    let (mut bootstrap_nodes, bootstrap_addresses) =
        create_bootstrap_nodes::<V>(bootstrap_count, validator.clone()).await?;
    let mut client_nodes =
        create_nodes_with_peers::<V>(client_count, &bootstrap_addresses, validator.clone()).await?;
    let expected_peers = client_count + bootstrap_count - 1;
    // Wait a few, since nodes need to announce each other via Identify,
    // which adds their address to the routing table. Kick off
    // another bootstrap process after that.
    // @TODO Figure out if bootstrapping is needed after identify-exchange,
    // as that typically happens on a ~5 minute timer.
    wait_ms(700).await;
    swarm_command(&mut client_nodes, |c| c.bootstrap()).await?;

    // Wait for the peers to establish connections.
    await_or_timeout(
        2000,
        swarm_command(&mut client_nodes, |c| c.wait_for_peers(expected_peers)),
        format!("waiting for {} peers", expected_peers),
    )
    .await?;

    await_or_timeout(
        2000,
        swarm_command(&mut bootstrap_nodes, |c| c.wait_for_peers(expected_peers)),
        format!("waiting for {} peers", expected_peers),
    )
    .await?;
    Ok((bootstrap_nodes, client_nodes, bootstrap_addresses))
}
