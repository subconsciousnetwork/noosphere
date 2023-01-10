#![cfg(not(target_arch = "wasm32"))]
#![cfg(test)]
use async_trait::async_trait;
use noosphere_ns::dht::{AllowAllValidator, DHTError, DHTNetworkInfo, DHTNode, RecordValidator};
pub mod utils;
use noosphere_core::authority::generate_ed25519_key;

use ucan_key_support::ed25519::Ed25519KeyMaterial;
use utils::{create_nodes_with_peers, create_test_dht_config, initialize_network, swarm_command};

/// Testing a detached DHTNode as a server with no peers.
#[test_log::test(tokio::test)]
async fn test_dhtnode_base_case() -> Result<(), DHTError> {
    let node = DHTNode::new::<Ed25519KeyMaterial, AllowAllValidator>(
        &generate_ed25519_key(),
        create_test_dht_config(),
        None,
    )?;
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

    if node.bootstrap().await.is_err() {
        panic!("bootstrap() should succeed, even without peers to bootstrap.");
    }
    Ok(())
}

/// Tests many client nodes connecting to a single bootstrap node,
/// and ensuring clients become peers.
#[test_log::test(tokio::test)]
async fn test_dhtnode_bootstrap() -> Result<(), DHTError> {
    let num_clients = 5;
    let (mut bootstrap_nodes, mut client_nodes, _bootstrap_addresses) =
        initialize_network::<AllowAllValidator>(1, num_clients, None).await?;
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
    let (mut _bootstrap_nodes, mut client_nodes, _bootstrap_addresses) =
        initialize_network::<AllowAllValidator>(1, num_clients, None).await?;

    let client_a = client_nodes.pop().unwrap();
    let client_b = client_nodes.pop().unwrap();
    client_a.put_record(b"foo", b"bar").await?;
    let result = client_b.get_record(b"foo").await?;
    assert_eq!(result.key, b"foo");
    assert_eq!(result.value.expect("has value"), b"bar");
    Ok(())
}

/// Testing primitive start_providing/get_providers between two
/// non-bootstrap peers.
#[test_log::test(tokio::test)]
async fn test_dhtnode_providers() -> Result<(), DHTError> {
    let num_clients = 2;
    let (mut _bootstrap_nodes, mut client_nodes, _bootstrap_addresses) =
        initialize_network::<AllowAllValidator>(1, num_clients, None).await?;

    let client_a = client_nodes.pop().unwrap();
    let client_b = client_nodes.pop().unwrap();
    client_a.start_providing(b"foo").await?;

    let providers = client_b.get_providers(b"foo").await?;
    println!("{:#?}", providers);
    assert_eq!(providers.len(), 1);
    assert_eq!(&providers[0], client_a.peer_id());
    Ok(())
}

#[test_log::test(tokio::test)]
async fn test_dhtnode_validator() -> Result<(), DHTError> {
    #[derive(Clone)]
    struct MyValidator {}

    #[async_trait]
    impl RecordValidator for MyValidator {
        async fn validate(&mut self, data: &[u8]) -> bool {
            data == b"VALID"
        }
    }

    let num_clients = 2;
    let (_bootstrap_nodes, mut client_nodes, bootstrap_addresses) =
        initialize_network(1, num_clients, Some(MyValidator {})).await?;

    let unfiltered_client = {
        let node = create_nodes_with_peers::<AllowAllValidator>(1, &bootstrap_addresses, None)
            .await?
            .pop()
            .unwrap();
        node.bootstrap().await?;
        node.wait_for_peers(1).await?;
        node
    };

    let client_a = client_nodes.pop().unwrap();
    let client_b = client_nodes.pop().unwrap();

    client_a.put_record(b"foo_1", b"VALID").await?;
    let result = client_b.get_record(b"foo_1").await?;
    assert_eq!(
        result.value.expect("has value"),
        b"VALID",
        "validation allows valid records through"
    );

    assert!(
        client_a.put_record(b"foo_2", b"INVALID").await.is_err(),
        "setting a record validates locally"
    );

    // set a valid and an invalid record from the unfiltered client
    unfiltered_client.put_record(b"foo_3", b"VALID").await?;
    unfiltered_client.put_record(b"foo_4", b"INVALID").await?;

    let result = client_b.get_record(b"foo_3").await?;
    assert_eq!(
        result.value.expect("has value"),
        b"VALID",
        "validation allows valid records through"
    );

    assert!(
        client_b.get_record(b"foo_4").await?.value.is_none(),
        "invalid records are not retrieved from the network"
    );

    Ok(())
}
