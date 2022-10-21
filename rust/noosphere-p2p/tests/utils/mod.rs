#![cfg(test)]
use futures::future::try_join_all;
use libp2p::{self, Multiaddr};
use noosphere_p2p::dht::{DHTConfig, DHTError, DHTNode};
use rand::{thread_rng, Rng};
use std::future::Future;
use std::time::Duration;
use tokio;
//use tracing::*;

pub fn generate_multiaddr() -> Multiaddr {
    let mut addr = "/ip4/127.0.0.1"
        .parse::<Multiaddr>()
        .expect("Default IP address");
    addr.push(libp2p::multiaddr::Protocol::Tcp(
        thread_rng().gen_range(49152..65535),
    ));
    addr
}

pub async fn wait_ms(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

pub async fn await_or_timeout<T>(
    timeout_ms: u64,
    future: impl Future<Output = T>,
    message: String,
) -> T {
    tokio::select! {
        _ = wait_ms(timeout_ms) => { panic!("timed out: {}", message); }
        result = future => { result }
    }
}

pub fn create_test_config() -> DHTConfig {
    let mut config = DHTConfig::default();
    config.peer_dialing_interval = 1;
    config
}

pub async fn swarm_command<'a, TFuture, F, T, E>(
    nodes: &'a mut Vec<DHTNode>,
    func: F,
) -> Result<Vec<T>, E>
where
    F: FnMut(&'a mut DHTNode) -> TFuture,
    TFuture: Future<Output = Result<T, E>>,
{
    let futures: Vec<_> = nodes.iter_mut().map(func).collect();
    try_join_all(futures).await
}

pub fn create_client_nodes_with_bootstrap_peers(
    bootstrap_count: usize,
    client_count: usize,
) -> Result<(Vec<DHTNode>, Vec<DHTNode>), DHTError> {
    let bootstrap_nodes = create_bootstrap_nodes(bootstrap_count)?;
    let bootstrap_addresses: Vec<libp2p::Multiaddr> = bootstrap_nodes
        .iter()
        .map(|node| node.p2p_address().clone())
        .collect();

    let mut client_nodes: Vec<DHTNode> = vec![];
    for _ in 0..client_count {
        let mut config = create_test_config();
        config.listening_address = generate_multiaddr();
        config.bootstrap_peers = bootstrap_addresses.clone();
        client_nodes.push(DHTNode::new(config)?);
    }
    Ok((bootstrap_nodes, client_nodes))
}

/// Creates `count` bootstrap nodes, each node using all other
/// bootstrap nodes as bootstrap peers.
pub fn create_bootstrap_nodes(count: usize) -> Result<Vec<DHTNode>, DHTError> {
    let mut configs: Vec<(DHTConfig, Multiaddr)> = vec![];
    for _ in 0..count {
        let mut config = create_test_config();
        config.listening_address = generate_multiaddr();
        let (_peer_id, p2p_address) = DHTConfig::get_peer_id_and_address(&config);
        configs.push((config, p2p_address));
    }

    let mut handles: Vec<DHTNode> = vec![];
    let mut index = 0;
    for c in &configs {
        let mut config = c.0.to_owned();
        for i in 0..count {
            if i != index {
                config
                    .bootstrap_peers
                    .push(configs[i as usize].1.to_owned());
            }
        }
        handles.push(DHTNode::new(config)?);
        index += 1;
    }
    Ok(handles)
}

pub async fn initialize_network(
    bootstrap_count: usize,
    client_count: usize,
) -> Result<(Vec<DHTNode>, Vec<DHTNode>), DHTError> {
    let (mut bootstrap_nodes, mut client_nodes) =
        create_client_nodes_with_bootstrap_peers(bootstrap_count, client_count)?;
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
    Ok((bootstrap_nodes, client_nodes))
}
