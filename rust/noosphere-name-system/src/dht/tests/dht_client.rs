#![cfg(test)]
use crate::dht::{DHTClient, DHTConfig, DHTError, DHTNetworkInfo, DHTStatus};
use anyhow::{anyhow, Result};
use futures::future::try_join_all;
use libp2p::{self, Multiaddr};
use rand::random;
use std::{str, time::Duration};
use test_log;
use tokio;
use tracing::*;

macro_rules! swarm_command {
    ($clients:expr, $command:ident) => {{
        let futures: Vec<_> = $clients.iter_mut().map(|c| c.$command()).collect();
        try_join_all(futures)
    }};
}

fn create_config() -> DHTConfig {
    DHTConfig {
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
        clients.push(DHTClient::new(config));
        index += 1;
    }
    clients
}

#[test_log::test(tokio::test)]
async fn test_dhtclient_base_case() -> Result<()> {
    let mut client = DHTClient::new(create_config());

    assert_eq!(client.status(), DHTStatus::Inactive, "DHT is inactive");
    client.start().await?;
    assert_eq!(client.status(), DHTStatus::Active, "DHT is active");

    let info = client.network_info().await?;
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
        client.bootstrap().await?,
        (),
        "bootstrap() should succeed, even without peers to bootstrap."
    );

    client.stop().await?;
    assert_eq!(client.status(), DHTStatus::Inactive, "DHT is inactive");
    Ok(())
}

#[test_log::test(tokio::test)]
async fn test_dhtclient_simple() -> Result<()> {
    let mut clients = create_connected_clients(2);
    swarm_command!(clients, start).await?;
    swarm_command!(clients, bootstrap).await?;

    let infos = swarm_command!(clients, network_info).await?;
    trace!("{:#?}", infos);
    for info in infos {
        assert_eq!(info.num_peers, 1);
    }

    clients[0]
        .set_record(
            String::from("foo").into_bytes(),
            String::from("bar").into_bytes(),
        )
        .await?;
    let result = clients[1]
        .get_record(String::from("foo").into_bytes())
        .await?;
    assert_eq!(str::from_utf8(result.as_ref().unwrap()).unwrap(), "bar");
    Ok(())
}

/// Tests many clients connecting to a single
/// bootstrap node.
#[test_log::test(tokio::test)]
async fn test_dhtclient_bootstrap() -> Result<()> {
    let client_count = 5;
    let bootstrap_config = create_config();
    let bootstrap_address = bootstrap_config.p2p_address().unwrap();
    let mut bootstrap = DHTClient::new(bootstrap_config);
    bootstrap.start().await?;

    let mut clients: Vec<DHTClient> = vec![];
    for _ in 0..client_count {
        let mut config = create_config();
        config.bootstrap_peers.push(bootstrap_address.clone());
        let client = DHTClient::new(config);
        clients.push(client);
    }

    swarm_command!(clients, start).await?;
    swarm_command!(clients, bootstrap).await?;
    let infos = swarm_command!(clients, network_info).await?;

    for info in infos {
        assert_eq!(info.num_peers, 1);
    }

    Ok(())
}
