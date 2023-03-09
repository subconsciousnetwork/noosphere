#![cfg(not(target_arch = "wasm32"))]

#[macro_use]
extern crate tracing;

use anyhow::Result;
use noosphere_storage::{MemoryStorage, SphereDb};
use std::{net::TcpListener, sync::Arc, time::Duration};

use noosphere_cli::native::{
    commands::{key::key_create, sphere::sphere_create},
    workspace::Workspace,
};
use noosphere_core::{data::Did, tracing::initialize_tracing};
use noosphere_gateway::{start_gateway, GatewayScope};
use noosphere_ns::{
    dht::AllowAllValidator, helpers::generate_name_systems_network,
    server::start_name_system_api_server,
};
use noosphere_sphere::{
    HasMutableSphereContext, HasSphereContext, SphereContentWrite, SpherePetnameRead,
    SpherePetnameWrite, SphereSync,
};
use tokio::{sync::Mutex, task::JoinHandle};
use url::Url;

async fn start_gateway_for_workspace(
    workspace: &Workspace,
    client_sphere_identity: &Did,
    ipfs_url: &Url,
    ns_url: &Url,
) -> Result<(Url, JoinHandle<()>)> {
    let gateway_listener = TcpListener::bind("127.0.0.1:0")?;
    let gateway_address = gateway_listener.local_addr()?;
    let gateway_url = Url::parse(&format!(
        "http://{}:{}",
        gateway_address.ip(),
        gateway_address.port()
    ))?;

    let gateway_sphere_context = workspace.sphere_context().await?.clone();

    let client_sphere_identity = client_sphere_identity.clone();
    let ns_url = ns_url.clone();
    let ipfs_url = ipfs_url.clone();

    let join_handle = tokio::spawn(async move {
        start_gateway(
            gateway_listener,
            GatewayScope {
                identity: gateway_sphere_context.identity().await.unwrap(),
                counterpart: client_sphere_identity,
            },
            gateway_sphere_context,
            ipfs_url,
            ns_url,
            None,
        )
        .await
        .unwrap()
    });

    Ok((gateway_url, join_handle))
}

#[tokio::test]
async fn gateway_publishes_and_resolves_petnames_configured_by_the_client() {
    initialize_tracing();

    let ipfs_url = Url::parse("http://127.0.0.1:5001").unwrap();

    let ns_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let ns_address = ns_listener.local_addr().unwrap();
    let ns_url =
        Url::parse(format!("http://{}:{}", ns_address.ip(), ns_address.port()).as_str()).unwrap();
    let ns_task = tokio::spawn(async move {
        let db = SphereDb::new(&MemoryStorage::default()).await.unwrap();
        let (ns_node, _, _ns_data) = generate_name_systems_network(2, db, AllowAllValidator {})
            .await
            .unwrap();

        start_name_system_api_server(Arc::new(Mutex::new(ns_node)), ns_listener)
            .await
            .unwrap();
    });

    let (gateway_workspace, _gateway_temporary_directories) = Workspace::temporary().unwrap();
    let (client_workspace, _client_temporary_directories) = Workspace::temporary().unwrap();

    let (third_party_client_workspace, _third_party_temporary_directores) =
        Workspace::temporary().unwrap();
    let (third_party_gateway_workspace, _third_party_temporary_directores) =
        Workspace::temporary().unwrap();

    let gateway_key_name = "GATEWAY_KEY";
    let client_key_name = "CLIENT_KEY";
    let third_party_client_key_name = "THIRD_PARTY_CLIENT_KEY";
    let third_party_gateway_key_name = "THIRD_PARTY_GATEWAY_KEY";

    key_create(client_key_name, &client_workspace)
        .await
        .unwrap();
    key_create(gateway_key_name, &gateway_workspace)
        .await
        .unwrap();
    key_create(third_party_client_key_name, &third_party_client_workspace)
        .await
        .unwrap();
    key_create(third_party_gateway_key_name, &third_party_gateway_workspace)
        .await
        .unwrap();

    sphere_create(client_key_name, &client_workspace)
        .await
        .unwrap();
    sphere_create(gateway_key_name, &gateway_workspace)
        .await
        .unwrap();
    sphere_create(third_party_client_key_name, &third_party_client_workspace)
        .await
        .unwrap();
    sphere_create(third_party_gateway_key_name, &third_party_gateway_workspace)
        .await
        .unwrap();

    let (gateway_url, gateway_task) = start_gateway_for_workspace(
        &gateway_workspace,
        &client_workspace.sphere_identity().await.unwrap(),
        &ipfs_url,
        &ns_url,
    )
    .await
    .unwrap();

    let (third_party_gateway_url, third_party_gateway_task) = start_gateway_for_workspace(
        &third_party_gateway_workspace,
        &third_party_client_workspace
            .sphere_identity()
            .await
            .unwrap(),
        &ipfs_url,
        &ns_url,
    )
    .await
    .unwrap();

    let mut third_party_client_sphere_context =
        third_party_client_workspace.sphere_context().await.unwrap();

    let third_party_client_task = tokio::spawn(async move {
        third_party_client_sphere_context
            .lock()
            .await
            .configure_gateway_url(Some(&third_party_gateway_url))
            .await
            .unwrap();

        third_party_client_sphere_context
            .write("foo", "text/plain", "bar".as_ref(), None)
            .await
            .unwrap();
        let version = third_party_client_sphere_context.save(None).await.unwrap();
        third_party_client_sphere_context.sync().await.unwrap();

        tokio::time::sleep(Duration::from_secs(1)).await;

        version
    });

    let third_party_client_sphere_identity = third_party_client_workspace
        .sphere_identity()
        .await
        .unwrap();
    let mut client_sphere_context = client_workspace.sphere_context().await.unwrap();

    let client_task = tokio::spawn(async move {
        let expected_third_party_sphere_version = third_party_client_task.await.unwrap();

        client_sphere_context
            .lock()
            .await
            .configure_gateway_url(Some(&gateway_url))
            .await
            .unwrap();

        client_sphere_context
            .set_petname("thirdparty", Some(third_party_client_sphere_identity))
            .await
            .unwrap();
        client_sphere_context.save(None).await.unwrap();
        client_sphere_context.sync().await.unwrap();

        tokio::time::sleep(Duration::from_secs(1)).await;

        debug!("Syncing to receive resolved name...");
        client_sphere_context.sync().await.unwrap();

        let resolved_third_party_sphere_version = client_sphere_context
            .resolve_petname("thirdparty")
            .await
            .unwrap();

        assert_eq!(
            resolved_third_party_sphere_version,
            Some(expected_third_party_sphere_version)
        );

        ns_task.abort();
        gateway_task.abort();
        third_party_gateway_task.abort();
    });

    client_task.await.unwrap();
}
