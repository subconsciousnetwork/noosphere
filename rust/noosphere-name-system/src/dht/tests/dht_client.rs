#![cfg(test)]
use crate::dht::{DHTClient, DHTConfig, DHTError, DHTNetworkInfo, DHTStatus};
use anyhow::{anyhow, Result};
use futures::future::try_join_all;
use libp2p;
use rand::random;
use test_log;
use tokio;
use tokio::signal;
use tracing::*;

fn create_config() -> DHTConfig {
    DHTConfig {
        keypair: libp2p::identity::Keypair::generate_ed25519(),
        listening_address: Some(libp2p::multiaddr::Protocol::Memory(random::<u64>()).into()),
        ..Default::default()
    }
}

fn create_connected_clients(count: u8) -> Vec<DHTClient> {
    let mut configs: Vec<DHTConfig> = vec![];
    for _ in 0..count {
        configs.push(create_config());
    }

    let mut clients: Vec<DHTClient> = vec![];
    let mut index = 0;
    for c in &configs {
        let mut config = c.to_owned();
        for i in 0..count {
            if i != index {
                let peer_id = configs[i as usize].peer_id();
                let mut peer_address = configs[i as usize].listening_address.clone().unwrap();
                peer_address.push(libp2p::multiaddr::Protocol::P2p(peer_id.into()));
                config.bootstrap_peers.push(peer_address);
            }
        }
        trace!("Creating DHTConfig {:#?}", config);
        clients.push(DHTClient::new(config));
        index += 1;
    }
    clients
}

#[test_log::test(tokio::test)]
async fn test_dhtclient_single() -> Result<()> {
    let mut client = DHTClient::new(create_config());

    assert_eq!(client.status(), DHTStatus::Inactive, "DHT is inactive");
    client.start().await?;
    assert_eq!(client.status(), DHTStatus::Active, "DHT is active");

    let info = client.network_info().await?;
    assert_eq!(info.num_connections, 0);
    assert_eq!(info.num_pending, 0);
    assert_eq!(info.num_established, 0);
    assert_eq!(info.num_peers, 0);
    let res = client.bootstrap().await;

    assert!(
        match client.bootstrap().await {
            Err(DHTError::NoKnownPeers) => true,
            Err(DHTError::LibP2PBootstrapError(_)) => true,
            _ => false,
        },
        "Cannot bootstrap without peers"
    );

    client.stop().await?;
    assert_eq!(client.status(), DHTStatus::Inactive, "DHT is inactive");
    Ok(())
}

#[test_log::test(tokio::test)]
async fn test_dhtclient_simple() -> Result<()> {
    let (mut first, mut second) = {
        let mut clients = create_connected_clients(2);
        (clients.pop().unwrap(), clients.pop().unwrap())
    };

    try_join_all(vec![first.start(), second.start()]).await?;
    let infos = try_join_all(vec![first.bootstrap(), second.bootstrap()]).await?;
    for info in infos {
        assert_eq!(info.num_peers, 1);
        assert_eq!(info.num_connections, 2);
        assert_eq!(info.num_established, 2);
        assert_eq!(info.num_pending, 0);
    }

    let result = first
        .set_record(
            String::from("foo").into_bytes(),
            String::from("bar").into_bytes(),
        )
        .await?;
    info!("Results: {:#?}", result);
    //Err(anyhow!(""))
    Ok(())
}
