#![cfg(not(target_arch = "wasm32"))]

use noosphere::key::KeyStorage;
use noosphere_sphere::{
    HasMutableSphereContext, HasSphereContext, SphereContentRead, SphereContentWrite, SphereCursor,
    SphereSync, SyncRecovery,
};
use noosphere_storage::BlockStore;
use std::net::TcpListener;
use tokio::io::AsyncReadExt;
use tokio_stream::StreamExt;
use url::Url;

use noosphere_api::route::Route;
use noosphere_core::{authority::Authorization, data::ContentType};

use ucan::crypto::KeyMaterial;

use noosphere_cli::native::{
    commands::{
        auth::auth_add,
        key::key_create,
        sphere::{sphere_create, sphere_join},
    },
    workspace::Workspace,
};
use noosphere_core::tracing::initialize_tracing;
use noosphere_gateway::{start_gateway, GatewayScope};

#[tokio::test]
async fn gateway_tells_you_its_identity() {
    initialize_tracing(None);
    let (gateway_workspace, _gateway_temporary_directories) = Workspace::temporary().unwrap();
    let (client_workspace, _client_temporary_directories) = Workspace::temporary().unwrap();

    let gateway_key_name = "GATEWAY_KEY";
    let client_key_name = "CLIENT_KEY";

    key_create(client_key_name, &client_workspace)
        .await
        .unwrap();
    key_create(gateway_key_name, &gateway_workspace)
        .await
        .unwrap();

    sphere_create(client_key_name, &client_workspace)
        .await
        .unwrap();
    sphere_create(gateway_key_name, &gateway_workspace)
        .await
        .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let gateway_address = listener.local_addr().unwrap();

    let gateway_sphere_identity = gateway_workspace.sphere_identity().await.unwrap();
    let client_sphere_identity = client_workspace.sphere_identity().await.unwrap();

    let gateway_sphere_context = gateway_workspace.sphere_context().await.unwrap();

    let server_task = tokio::spawn({
        let gateway_sphere_identity = gateway_sphere_identity.clone();
        async move {
            start_gateway(
                listener,
                GatewayScope {
                    identity: gateway_sphere_identity,
                    counterpart: client_sphere_identity,
                },
                gateway_sphere_context,
                Url::parse("http://127.0.0.1:5001").unwrap(),
                Url::parse("http://127.0.0.1:6667").unwrap(),
                None,
            )
            .await
            .unwrap()
        }
    });

    let gateway_identity = gateway_workspace
        .key()
        .await
        .unwrap()
        .get_did()
        .await
        .unwrap();

    let client = reqwest::Client::new();

    let mut url = Url::parse(&format!(
        "http://{}:{}",
        gateway_address.ip(),
        gateway_address.port()
    ))
    .unwrap();
    url.set_path(&Route::Did.to_string());

    let did_response = client.get(url).send().await.unwrap().text().await.unwrap();

    assert_eq!(gateway_identity, did_response);

    server_task.abort();
}

#[tokio::test]
async fn gateway_identity_can_be_verified_by_the_client_of_its_owner() {
    initialize_tracing(None);

    let (gateway_workspace, _gateway_temporary_directories) = Workspace::temporary().unwrap();
    let (client_workspace, _client_temporary_directories) = Workspace::temporary().unwrap();

    let gateway_key_name = "GATEWAY_KEY";
    let client_key_name = "CLIENT_KEY";

    key_create(client_key_name, &client_workspace)
        .await
        .unwrap();
    key_create(gateway_key_name, &gateway_workspace)
        .await
        .unwrap();

    sphere_create(client_key_name, &client_workspace)
        .await
        .unwrap();
    sphere_create(gateway_key_name, &gateway_workspace)
        .await
        .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let gateway_address = listener.local_addr().unwrap();

    let gateway_sphere_identity = gateway_workspace.sphere_identity().await.unwrap();
    let client_sphere_identity = client_workspace.sphere_identity().await.unwrap();

    let gateway_sphere_context = gateway_workspace.sphere_context().await.unwrap();

    let server_task = tokio::spawn({
        let gateway_sphere_identity = gateway_sphere_identity.clone();
        async move {
            start_gateway(
                listener,
                GatewayScope {
                    identity: gateway_sphere_identity,
                    counterpart: client_sphere_identity,
                },
                gateway_sphere_context,
                Url::parse("http://127.0.0.1:5001").unwrap(),
                Url::parse("http://127.0.0.1:6667").unwrap(),
                None,
            )
            .await
            .unwrap()
        }
    });

    let client_sphere_context = client_workspace.sphere_context().await.unwrap();
    let gateway_identity = gateway_workspace
        .key()
        .await
        .unwrap()
        .get_did()
        .await
        .unwrap();

    let client_task = tokio::spawn(async move {
        let mut client_sphere_context = client_sphere_context.lock().await;

        client_sphere_context
            .configure_gateway_url(Some(
                &format!("http://{}:{}", gateway_address.ip(), gateway_address.port())
                    .parse()
                    .unwrap(),
            ))
            .await
            .unwrap();

        let client = client_sphere_context.client().await.unwrap();

        assert_eq!(client.session.gateway_identity, gateway_identity);

        server_task.abort();
        let _ = server_task.await;
    });

    client_task.await.unwrap();
}

#[tokio::test]
async fn gateway_receives_a_newly_initialized_sphere_from_the_client() {
    initialize_tracing(None);

    let (gateway_workspace, _gateway_temporary_directories) = Workspace::temporary().unwrap();
    let (client_workspace, _client_temporary_directories) = Workspace::temporary().unwrap();

    let gateway_key_name = "GATEWAY_KEY";
    let client_key_name = "CLIENT_KEY";

    key_create(client_key_name, &client_workspace)
        .await
        .unwrap();
    key_create(gateway_key_name, &gateway_workspace)
        .await
        .unwrap();

    sphere_create(client_key_name, &client_workspace)
        .await
        .unwrap();
    sphere_create(gateway_key_name, &gateway_workspace)
        .await
        .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let gateway_address = listener.local_addr().unwrap();

    let gateway_sphere_identity = gateway_workspace.sphere_identity().await.unwrap();
    let client_sphere_identity = client_workspace.sphere_identity().await.unwrap();

    let gateway_sphere_context = gateway_workspace.sphere_context().await.unwrap();

    let server_task = {
        let gateway_sphere_context = gateway_sphere_context.clone();
        let client_sphere_identity = client_sphere_identity.clone();
        tokio::spawn(async move {
            start_gateway(
                listener,
                GatewayScope {
                    identity: gateway_sphere_identity,
                    counterpart: client_sphere_identity,
                },
                gateway_sphere_context,
                Url::parse("http://127.0.0.1:5001").unwrap(),
                Url::parse("http://127.0.0.1:6667").unwrap(),
                None,
            )
            .await
            .unwrap()
        })
    };

    let mut client_sphere_context = client_workspace.sphere_context().await.unwrap();

    let client_task = tokio::spawn(async move {
        {
            let mut client_sphere_context = client_sphere_context.lock().await;

            client_sphere_context
                .configure_gateway_url(Some(
                    &format!("http://{}:{}", gateway_address.ip(), gateway_address.port())
                        .parse()
                        .unwrap(),
                ))
                .await
                .unwrap();
        }

        let sphere_cid = client_sphere_context
            .sync(SyncRecovery::None)
            .await
            .unwrap();
        let db = client_sphere_context
            .sphere_context()
            .await
            .unwrap()
            .db()
            .clone();

        let block_stream = db.stream_links(&sphere_cid);

        tokio::pin!(block_stream);

        let gateway_db = {
            let gateway_sphere_context = gateway_sphere_context.lock().await;
            gateway_sphere_context.db().clone()
        };

        while let Some(cid) = block_stream.try_next().await.unwrap() {
            assert!(gateway_db.get_block(&cid).await.unwrap().is_some());
        }

        server_task.abort();

        let _ = server_task.await;
    });

    client_task.await.unwrap();
}

#[tokio::test]
async fn gateway_updates_an_existing_sphere_with_changes_from_the_client() {
    initialize_tracing(None);

    let (gateway_workspace, _gateway_temporary_directories) = Workspace::temporary().unwrap();
    let (client_workspace, _client_temporary_directories) = Workspace::temporary().unwrap();

    let gateway_key_name = "GATEWAY_KEY";
    let client_key_name = "CLIENT_KEY";

    key_create(client_key_name, &client_workspace)
        .await
        .unwrap();
    key_create(gateway_key_name, &gateway_workspace)
        .await
        .unwrap();

    sphere_create(client_key_name, &client_workspace)
        .await
        .unwrap();
    sphere_create(gateway_key_name, &gateway_workspace)
        .await
        .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let gateway_address = listener.local_addr().unwrap();

    let gateway_key = gateway_workspace.key().await.unwrap();
    let gateway_identity = gateway_key.get_did().await.unwrap();

    let gateway_sphere_identity = gateway_workspace.sphere_identity().await.unwrap();
    let client_sphere_identity = client_workspace.sphere_identity().await.unwrap();

    let gateway_sphere_context = gateway_workspace.sphere_context().await.unwrap();

    let server_task = {
        let gateway_sphere_context = gateway_sphere_context.clone();
        let client_sphere_identity = client_sphere_identity.clone();
        tokio::spawn(async move {
            start_gateway(
                listener,
                GatewayScope {
                    identity: gateway_sphere_identity,
                    counterpart: client_sphere_identity,
                },
                gateway_sphere_context,
                Url::parse("http://127.0.0.1:5001").unwrap(),
                Url::parse("http://127.0.0.1:6667").unwrap(),
                None,
            )
            .await
            .unwrap()
        })
    };

    let mut client_sphere_context = client_workspace.sphere_context().await.unwrap();

    let client_task = tokio::spawn(async move {
        {
            let mut client_sphere_context = client_sphere_context.lock().await;

            client_sphere_context
                .configure_gateway_url(Some(
                    &format!("http://{}:{}", gateway_address.ip(), gateway_address.port())
                        .parse()
                        .unwrap(),
                ))
                .await?;

            assert_eq!(
                client_sphere_context.gateway_identity().await?,
                gateway_identity
            );
        }

        let _ = client_sphere_context.sync(SyncRecovery::None).await?;

        for value in ["one", "two", "three"] {
            client_sphere_context
                .write(value, &ContentType::Text, value.as_bytes(), None)
                .await?;
            client_sphere_context.save(None).await?;
        }

        let sphere_cid = client_sphere_context.sync(SyncRecovery::None).await?;

        let db = client_sphere_context.sphere_context().await?.db().clone();
        let block_stream = db.stream_links(&sphere_cid);

        tokio::pin!(block_stream);

        let gateway_db = {
            let gateway_sphere_context = gateway_sphere_context.lock().await;
            gateway_sphere_context.db().clone()
        };

        while let Some(cid) = block_stream.try_next().await? {
            assert!(gateway_db.get_block(&cid).await?.is_some());
        }

        server_task.abort();
        let _ = server_task.await;

        Ok(()) as Result<_, anyhow::Error>
    });

    client_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn gateway_serves_sphere_revisions_to_a_client() {
    initialize_tracing(None);

    let (gateway_workspace, _gateway_temporary_directories) = Workspace::temporary().unwrap();
    let (client_workspace, _client_temporary_directories) = Workspace::temporary().unwrap();

    let gateway_key_name = "GATEWAY_KEY";
    let client_key_name = "CLIENT_KEY";

    key_create(client_key_name, &client_workspace)
        .await
        .unwrap();
    key_create(gateway_key_name, &gateway_workspace)
        .await
        .unwrap();

    sphere_create(client_key_name, &client_workspace)
        .await
        .unwrap();
    sphere_create(gateway_key_name, &gateway_workspace)
        .await
        .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let gateway_address = listener.local_addr().unwrap();

    let gateway_key = gateway_workspace.key().await.unwrap();
    let gateway_identity = gateway_key.get_did().await.unwrap();

    let gateway_sphere_identity = gateway_workspace.sphere_identity().await.unwrap();
    let client_sphere_identity = client_workspace.sphere_identity().await.unwrap();

    let gateway_sphere_context = gateway_workspace.sphere_context().await.unwrap();

    let server_task = {
        let gateway_sphere_context = gateway_sphere_context.clone();
        let client_sphere_identity = client_sphere_identity.clone();
        tokio::spawn(async move {
            start_gateway(
                listener,
                GatewayScope {
                    identity: gateway_sphere_identity,
                    counterpart: client_sphere_identity,
                },
                gateway_sphere_context,
                Url::parse("http://127.0.0.1:5001").unwrap(),
                Url::parse("http://127.0.0.1:6667").unwrap(),
                None,
            )
            .await
            .unwrap()
        })
    };

    let mut client_sphere_context = client_workspace.sphere_context().await.unwrap();

    let client_task = tokio::spawn(async move {
        {
            let mut client_sphere_context = client_sphere_context.lock().await;

            client_sphere_context
                .configure_gateway_url(Some(
                    &format!("http://{}:{}", gateway_address.ip(), gateway_address.port())
                        .parse()
                        .unwrap(),
                ))
                .await
                .unwrap();

            assert_eq!(
                client_sphere_context.gateway_identity().await.unwrap(),
                gateway_identity
            );
        }

        for value in ["one", "two", "three"] {
            client_sphere_context
                .write(value, &ContentType::Text, value.as_bytes(), None)
                .await?;
            client_sphere_context.save(None).await?;
        }

        client_sphere_context.sync(SyncRecovery::None).await?;

        server_task.abort();
        let _ = server_task.await;
        Ok(()) as Result<_, anyhow::Error>
    });

    client_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn gateway_can_sync_an_authorized_sphere_across_multiple_replicas() {
    initialize_tracing(None);

    let (gateway_workspace, _gateway_temporary_directories) = Workspace::temporary().unwrap();
    let (client_workspace, _client_temporary_directories) = Workspace::temporary().unwrap();
    let (client_replica_workspace, _client_replica_temporary_directories) =
        Workspace::temporary().unwrap();

    let gateway_key_name = "GATEWAY_KEY";
    let client_key_name = "CLIENT_KEY";
    let client_replica_key_name = "CLIENT_REPLICA_KEY";

    key_create(client_key_name, &client_workspace)
        .await
        .unwrap();
    key_create(gateway_key_name, &gateway_workspace)
        .await
        .unwrap();
    key_create(client_replica_key_name, &client_replica_workspace)
        .await
        .unwrap();

    sphere_create(client_key_name, &client_workspace)
        .await
        .unwrap();
    sphere_create(gateway_key_name, &gateway_workspace)
        .await
        .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let gateway_address = listener.local_addr().unwrap();

    let gateway_sphere_identity = gateway_workspace.sphere_identity().await.unwrap();
    let client_sphere_identity = client_workspace.sphere_identity().await.unwrap();

    let gateway_sphere_context = gateway_workspace.sphere_context().await.unwrap();

    let server_task = {
        let gateway_sphere_context = gateway_sphere_context.clone();
        let client_sphere_identity = client_sphere_identity.clone();
        tokio::spawn(async move {
            start_gateway(
                listener,
                GatewayScope {
                    identity: gateway_sphere_identity,
                    counterpart: client_sphere_identity,
                },
                gateway_sphere_context,
                Url::parse("http://127.0.0.1:5001").unwrap(),
                Url::parse("http://127.0.0.1:6667").unwrap(),
                None,
            )
            .await
            .unwrap()
        })
    };

    let client_replica_key_storage = client_replica_workspace.key_storage();
    let client_replica_key = client_replica_key_storage
        .require_key(client_replica_key_name)
        .await
        .unwrap();

    let client_replica_authorization = Authorization::Cid(
        auth_add(
            &client_replica_key.get_did().await.unwrap(),
            None,
            &client_workspace,
        )
        .await
        .unwrap(),
    );

    sphere_join(
        client_replica_key_name,
        Some(client_replica_authorization.to_string()),
        &client_sphere_identity,
        &client_replica_workspace,
    )
    .await
    .unwrap();

    let mut client_sphere_context = client_workspace.sphere_context().await.unwrap();
    let mut client_replica_sphere_context =
        client_replica_workspace.sphere_context().await.unwrap();

    let client_task = tokio::spawn(async move {
        let gateway_url: Url =
            format!("http://{}:{}", gateway_address.ip(), gateway_address.port())
                .parse()
                .unwrap();

        {
            client_sphere_context
                .lock()
                .await
                .configure_gateway_url(Some(&gateway_url))
                .await
                .unwrap();
        }

        for value in ["one", "two", "three"] {
            client_sphere_context
                .write(value, &ContentType::Subtext, value.as_ref(), None)
                .await
                .unwrap();
            SphereCursor::latest(client_sphere_context.clone())
                .save(None)
                .await
                .unwrap();
        }

        client_sphere_context
            .sync(SyncRecovery::None)
            .await
            .unwrap();

        {
            let mut client_replica_sphere_context = client_replica_sphere_context.lock().await;
            client_replica_sphere_context
                .configure_gateway_url(Some(&gateway_url))
                .await
                .unwrap();
        }
        client_replica_sphere_context
            .sync(SyncRecovery::None)
            .await
            .unwrap();

        for value in ["one", "two", "three"] {
            let mut file = client_replica_sphere_context
                .read(value)
                .await
                .unwrap()
                .unwrap();
            let mut contents = String::new();
            file.contents.read_to_string(&mut contents).await.unwrap();
            assert_eq!(value, &contents);
        }

        server_task.abort();
        let _ = server_task.await;
    });

    client_task.await.unwrap();
}
