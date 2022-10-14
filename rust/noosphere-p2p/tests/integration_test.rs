#![cfg(test)]
use noosphere_p2p::dht::{
    spawn_dht_node, DHTConfig, DHTError, DHTNetworkInfo, DHTNodeHandle, DHTStatus,
};
use std::str;
use test_log;
use tokio;
use tracing::*;
mod utils;
use utils::{
    await_or_timeout, create_bootstrap_nodes, create_client_nodes_with_bootstrap_peers,
    create_test_config, initialize_network, memory_multiaddr, swarm_command, wait_ms,
};

/// Testing a detached DHTNode as a server with no peers.
#[test_log::test(tokio::test)]
async fn test_dhtnode_base_case() -> Result<(), DHTError> {
    let mut handle = spawn_dht_node(create_test_config().listening_address(memory_multiaddr()))?;

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

/// Tests many client nodes connecting to a single bootstrap node,
/// and ensuring clients become peers.
#[test_log::test(tokio::test)]
async fn test_dhtnode_bootstrap() -> Result<(), DHTError> {
    let num_clients = 5;
    let (mut bootstrap_nodes, mut client_nodes) = initialize_network(1, num_clients).await?;
    let bootstrap = bootstrap_nodes.pop().unwrap();

    for info in swarm_command(&mut client_nodes, |c| c.network_info()).await? {
        assert_eq!(info.num_peers, num_clients);
        // @TODO the number of connections seem inconsistent??
        //assert_eq!(info.num_connections, num_clients as u32);
        //assert_eq!(info.num_established, num_clients as u32);
        assert_eq!(info.num_pending, 0);
    }

    let info = bootstrap.network_info().await?;
    assert_eq!(info.num_peers, num_clients);
    // @TODO the number of connections seem inconsistent??
    //assert_eq!(info.num_connections, num_clients as u32);
    //assert_eq!(info.num_established, num_clients as u32);
    assert_eq!(info.num_pending, 0);

    Ok(())
}

/// Testing primitive set_record/get_record between two
/// non-bootstrap peers.
#[test_log::test(tokio::test)]
async fn test_dhtnode_simple() -> Result<(), DHTError> {
    let num_clients = 2;
    let (mut _bootstrap_nodes, mut client_nodes) = initialize_network(1, num_clients).await?;

    let client_a = client_nodes.pop().unwrap();
    let client_b = client_nodes.pop().unwrap();
    client_a
        .set_record(
            String::from("foo").into_bytes(),
            String::from("bar").into_bytes(),
        )
        .await?;
    let result = client_b
        .get_record(String::from("foo").into_bytes())
        .await?;
    assert_eq!(str::from_utf8(result.as_ref().unwrap()).unwrap(), "bar");
    Ok(())
}

/// Testing primitive start_providing/get_providers between two
/// non-bootstrap peers.
#[test_log::test(tokio::test)]
async fn test_dhtnode_providers() -> Result<(), DHTError> {
    let num_clients = 2;
    let (mut _bootstrap_nodes, mut client_nodes) = initialize_network(1, num_clients).await?;

    let client_a = client_nodes.pop().unwrap();
    let client_b = client_nodes.pop().unwrap();
    client_a.start_providing(Vec::from("foo")).await?;

    let providers = client_b.get_providers(Vec::from("foo")).await?;
    println!("{:#?}", providers);
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0], client_a.peer_id());
    Ok(())
}
