#![cfg(test)]
use futures::future::try_join_all;
use libp2p::{self, Multiaddr};
use noosphere_p2p::dht::{
    spawn_dht_node, DHTConfig, DHTError, DHTNetworkInfo, DHTNodeHandle, DHTStatus,
};
use rand::random;
use std::future::Future;
use std::{str, time::Duration};
use test_log;
use tokio;
use tracing::*;

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
    DHTConfig::default().peer_dialing_interval(1)
}

pub async fn swarm_command<'a, P, F, T, E>(
    nodes: &'a mut Vec<DHTNodeHandle>,
    func: F,
) -> Result<Vec<T>, E>
where
    F: FnMut(&'a mut DHTNodeHandle) -> P,
    P: Future<Output = Result<T, E>>,
{
    let futures: Vec<_> = nodes.iter_mut().map(func).collect();
    try_join_all(futures).await
}

pub fn memory_multiaddr() -> Multiaddr {
    Multiaddr::from(libp2p::multiaddr::Protocol::Memory(random::<u64>()))
}

pub fn create_client_nodes_with_bootstrap_peers(
    bootstrap_count: usize,
    client_count: usize,
) -> Result<(Vec<DHTNodeHandle>, Vec<DHTNodeHandle>), DHTError> {
    let bootstrap_nodes = create_bootstrap_nodes(bootstrap_count)?;
    let bootstrap_addresses: Vec<libp2p::Multiaddr> = bootstrap_nodes
        .iter()
        .map(|node| node.p2p_address())
        .collect();

    let mut client_nodes: Vec<DHTNodeHandle> = vec![];
    for _ in 0..client_count {
        let config = create_test_config()
            .listening_address(memory_multiaddr())
            .bootstrap_peers(bootstrap_addresses.clone());
        client_nodes.push(spawn_dht_node(config)?);
    }
    Ok((bootstrap_nodes, client_nodes))
}

/// Creates `count` bootstrap nodes, each node using all other
/// bootstrap nodes as bootstrap peers.
pub fn create_bootstrap_nodes(count: usize) -> Result<Vec<DHTNodeHandle>, DHTError> {
    let mut configs: Vec<DHTConfig> = vec![];
    for _ in 0..count {
        configs.push(create_test_config().listening_address(memory_multiaddr()));
    }

    let mut handles: Vec<DHTNodeHandle> = vec![];
    let mut index = 0;
    for c in &configs {
        let mut config = c.to_owned();
        for i in 0..count {
            if i != index {
                config
                    .bootstrap_peers
                    .push(configs[i as usize].p2p_address());
            }
        }
        handles.push(spawn_dht_node(config)?);
        index += 1;
    }
    Ok(handles)
}

pub async fn initialize_network(
    bootstrap_count: usize,
    client_count: usize,
) -> Result<(Vec<DHTNodeHandle>, Vec<DHTNodeHandle>), DHTError> {
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
