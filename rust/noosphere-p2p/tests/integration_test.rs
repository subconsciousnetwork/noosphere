#![cfg(test)]
use futures::future::try_join_all;
use libp2p::{self, Multiaddr};
use noosphere_p2p::dht::{
    spawn_dht_node, DHTConfig, DHTError, DHTNetworkInfo, DHTNodeHandle, DHTStatus,
};
use rand::random;
use std::{str, time::Duration};
use test_log;
use tokio;
use tracing::*;

macro_rules! swarm_command {
    ($handles:expr, $command:ident $(, $args:expr)* ) => {{
        println!("FOO swarm command with args");
        let futures: Vec<_> = $handles.iter_mut().map(|c| c.$command($($args,)*)).collect();
        try_join_all(futures)
    }};
    /*
    ($handles:expr, |$arg:ident| $body:expr) => {{
        println!("FOO swarm command with args");
        let futures: Vec<_> = $handles.iter_mut().map(move |$arg| $body).collect();
        try_join_all(futures)
    }};
    */
     /*
        ($handles:expr, $command:ident) => {{
            println!("FOO swarm command with no args");
            let futures: Vec<_> = $handles.iter_mut().map(|c| c.$command()).collect();
            try_join_all(futures)
        }};
        */
}

async fn wait_ms(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

fn memory_multiaddr() -> Multiaddr {
    Multiaddr::from(libp2p::multiaddr::Protocol::Memory(random::<u64>()))
}

fn create_client_nodes_with_bootstrap_peers(
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
        let config = DHTConfig::default()
            .listening_address(memory_multiaddr())
            .bootstrap_peers(bootstrap_addresses.clone());
        client_nodes.push(spawn_dht_node(config)?);
    }
    Ok((bootstrap_nodes, client_nodes))
}

/// Creates `count` bootstrap nodes, each node using all other
/// bootstrap nodes as bootstrap peers.
fn create_bootstrap_nodes(count: usize) -> Result<Vec<DHTNodeHandle>, DHTError> {
    let mut configs: Vec<DHTConfig> = vec![];
    for _ in 0..count {
        configs.push(DHTConfig::default().listening_address(memory_multiaddr()));
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

/// Testing a detached DHTNode as a server with no peers.
#[test_log::test(tokio::test)]
async fn test_dhtnode_base_case() -> Result<(), DHTError> {
    let mut handle = spawn_dht_node(DHTConfig::default().listening_address(memory_multiaddr()))?;

    assert_eq!(handle.status(), DHTStatus::Active, "DHT is active");

    let info = handle.network_info().await?;
    assert_eq!(
        info,
        DHTNetworkInfo {
            num_connections: 0,
            num_established: 0,
            num_peers: 0,
            num_pending: 0,
        }
    );
    assert_eq!(
        handle.bootstrap().await?,
        (),
        "bootstrap() should succeed, even without peers to bootstrap."
    );

    handle.terminate()?;
    assert_eq!(handle.status(), DHTStatus::Terminated, "DHT is terminated");
    Ok(())
}

/// Testing primitive set_record/get_record.
#[test_log::test(tokio::test)]
async fn test_dhtnode_simple() -> Result<(), DHTError> {
    let mut handles = create_bootstrap_nodes(2)?;

    swarm_command!(handles, wait_for_peers, 1).await?;
    for info in swarm_command!(handles, network_info).await? {
        assert_eq!(info.num_peers, 1);
    }

    handles[0]
        .set_record(
            String::from("foo").into_bytes(),
            String::from("bar").into_bytes(),
        )
        .await?;
    let result = handles[1]
        .get_record(String::from("foo").into_bytes())
        .await?;
    assert_eq!(str::from_utf8(result.as_ref().unwrap()).unwrap(), "bar");
    Ok(())
}

/// Tests many client nodes connecting to a single bootstrap node.
#[test_log::test(tokio::test)]
async fn test_dhtnode_bootstrap() -> Result<(), DHTError> {
    let num_clients = 5;
    let (mut bootstrap_nodes, mut client_nodes) =
        create_client_nodes_with_bootstrap_peers(1, num_clients)?;
    let bootstrap = bootstrap_nodes.pop().unwrap();

    swarm_command!(client_nodes, bootstrap).await?;
    swarm_command!(client_nodes, wait_for_peers, 1).await?;

    for info in swarm_command!(client_nodes, network_info).await? {
        assert_eq!(info.num_peers, 1);
        assert_eq!(info.num_connections, 1);
        assert_eq!(info.num_established, 1);
    }

    bootstrap.wait_for_peers(num_clients).await?;
    assert_eq!(
        bootstrap.network_info().await?,
        DHTNetworkInfo {
            num_connections: num_clients as u32,
            num_established: num_clients as u32,
            num_peers: num_clients,
            num_pending: 0,
        },
        "bootstrap node has expected peers"
    );

    {
        let first = client_nodes.first_mut().unwrap();
        first.set_record(Vec::from("foo"), Vec::from("bar")).await?;
    }
    {
        let last = client_nodes.last_mut().unwrap();
        let result = last.get_record(Vec::from("foo")).await?.unwrap();
        assert_eq!(
            result,
            Vec::from("bar"),
            "value fetched from DHT matches name"
        );
    }

    Ok(())
}

#[test_log::test(tokio::test)]
async fn test_dhtnode_providers() -> Result<(), DHTError> {
    println!("FOO1");
    let num_clients = 2;
    let (mut bootstrap_nodes, mut client_nodes) =
        create_client_nodes_with_bootstrap_peers(1, num_clients)?;
    let bootstrap = bootstrap_nodes.pop().unwrap();
    let client_a = client_nodes.pop().unwrap();
    let client_b = client_nodes.pop().unwrap();
    println!("FOO2");

    swarm_command!(client_nodes, bootstrap).await?;
    swarm_command!(client_nodes, wait_for_peers, 3).await?;
    println!("FOO3");
    bootstrap.wait_for_peers(num_clients).await?;

    let info = client_a.network_info().await?;
    assert_eq!(info.num_peers, 3);
    client_a.start_providing(Vec::from("foo")).await?;
    //let providers = client_b.get_providers(Vec::from("foo")).await?;
    //println!("{:#?}", providers);
    //Err(DHTError::Error("foo".into()))
    Ok(())
}
