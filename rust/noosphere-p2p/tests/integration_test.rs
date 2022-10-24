#![cfg(not(target_arch = "wasm32"))]
#![cfg(test)]
use noosphere_p2p::dht::{DHTError, DHTNetworkInfo, DHTNode, DHTStatus};
use std::str;
use test_log;
use tokio;
//use tracing::*;
mod utils;
use utils::{create_test_config, generate_multiaddr, initialize_network, swarm_command};

/// Testing a detached DHTNode as a server with no peers.
#[test_log::test(tokio::test)]
async fn test_dhtnode_base_case() -> Result<(), DHTError> {
    let mut config = create_test_config();
    config.listening_address = generate_multiaddr();
    let mut node = DHTNode::new(config)?;

    assert_eq!(node.status(), DHTStatus::Active, "DHT is active");

    let info = node.network_info().await?;
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
        node.bootstrap().await?,
        (),
        "bootstrap() should succeed, even without peers to bootstrap."
    );

    node.terminate()?;
    assert_eq!(node.status(), DHTStatus::Terminated, "DHT is terminated");
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
        // TODO(#100) the number of connections seem inconsistent??
        //assert_eq!(info.num_connections, num_clients as u32);
        //assert_eq!(info.num_established, num_clients as u32);
        assert_eq!(info.num_pending, 0);
    }

    let info = bootstrap.network_info().await?;
    assert_eq!(info.num_peers, num_clients);
    // TODO(#100) the number of connections seem inconsistent??
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
    assert_eq!(&providers[0], client_a.peer_id());
    Ok(())
}
