#![cfg(not(target_arch = "wasm32"))]

//! Integration tests that span the distance between a client and a gateway;
//! tests in this module should be able to run without an available backing
//! IPFS-like block syndication layer.

use anyhow::Result;
use noosphere::key::KeyStorage;
use noosphere_core::context::{
    HasMutableSphereContext, HasSphereContext, SphereAuthorityWrite, SphereContentRead,
    SphereContentWrite, SphereCursor, SphereSync, SyncRecovery,
};
use noosphere_storage::BlockStore;
use std::net::TcpListener;
use tokio::io::AsyncReadExt;
use tokio_stream::StreamExt;
use url::Url;

use noosphere_core::api::v0alpha1;
use noosphere_core::data::{ContentType, Did};

use ucan::crypto::KeyMaterial;

use noosphere_cli::{
    commands::{
        key::key_create,
        sphere::{sphere_create, sphere_join},
    },
    helpers::{temporary_workspace, SpherePair},
};
use noosphere_core::tracing::initialize_tracing;
use noosphere_gateway::{start_gateway, GatewayScope};

#[tokio::test]
async fn gateway_tells_you_its_identity() -> Result<()> {
    initialize_tracing(None);
    let (mut gateway_workspace, _gateway_temporary_directories) = temporary_workspace()?;
    let (mut client_workspace, _client_temporary_directories) = temporary_workspace()?;

    let gateway_key_name = "GATEWAY_KEY";
    let client_key_name = "CLIENT_KEY";

    key_create(client_key_name, &client_workspace).await?;
    key_create(gateway_key_name, &gateway_workspace).await?;

    sphere_create(client_key_name, &mut client_workspace).await?;
    sphere_create(gateway_key_name, &mut gateway_workspace).await?;

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

    let gateway_identity = gateway_workspace.author().await?.did().await?;

    let client = reqwest::Client::new();

    let mut url = Url::parse(&format!(
        "http://{}:{}",
        gateway_address.ip(),
        gateway_address.port()
    ))
    .unwrap();
    url.set_path(&v0alpha1::Route::Did.to_string());

    let did_response = client.get(url).send().await.unwrap().text().await.unwrap();

    assert_eq!(gateway_identity, did_response);

    server_task.abort();

    Ok(())
}

#[tokio::test]
async fn gateway_identity_can_be_verified_by_the_client_of_its_owner() -> Result<()> {
    initialize_tracing(None);

    let (mut gateway_workspace, _gateway_temporary_directories) = temporary_workspace()?;
    let (mut client_workspace, _client_temporary_directories) = temporary_workspace()?;

    let gateway_key_name = "GATEWAY_KEY";
    let client_key_name = "CLIENT_KEY";

    key_create(client_key_name, &client_workspace).await?;
    key_create(gateway_key_name, &gateway_workspace).await?;

    sphere_create(client_key_name, &mut client_workspace).await?;
    sphere_create(gateway_key_name, &mut gateway_workspace).await?;

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
    let gateway_identity = gateway_workspace.author().await?.did().await?;

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

    Ok(())
}

#[tokio::test]
async fn gateway_receives_a_newly_initialized_sphere_from_the_client() -> Result<()> {
    initialize_tracing(None);

    let (mut gateway_workspace, _gateway_temporary_directories) = temporary_workspace()?;
    let (mut client_workspace, _client_temporary_directories) = temporary_workspace()?;

    let gateway_key_name = "GATEWAY_KEY";
    let client_key_name = "CLIENT_KEY";

    key_create(client_key_name, &client_workspace).await?;
    key_create(gateway_key_name, &gateway_workspace).await?;

    sphere_create(client_key_name, &mut client_workspace).await?;
    sphere_create(gateway_key_name, &mut gateway_workspace).await?;

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
    Ok(())
}

#[tokio::test]
async fn gateway_updates_an_existing_sphere_with_changes_from_the_client() -> Result<()> {
    initialize_tracing(None);

    let (mut gateway_workspace, _gateway_temporary_directories) = temporary_workspace()?;
    let (mut client_workspace, _client_temporary_directories) = temporary_workspace()?;

    let gateway_key_name = "GATEWAY_KEY";
    let client_key_name = "CLIENT_KEY";

    key_create(client_key_name, &client_workspace).await?;
    key_create(gateway_key_name, &gateway_workspace).await?;

    sphere_create(client_key_name, &mut client_workspace).await?;
    sphere_create(gateway_key_name, &mut gateway_workspace).await?;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let gateway_address = listener.local_addr().unwrap();

    let gateway_identity = gateway_workspace.author().await?.did().await?;

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

    client_task.await??;
    Ok(())
}

#[tokio::test]
async fn gateway_receives_sphere_revisions_from_a_client() -> Result<()> {
    initialize_tracing(None);

    let mut sphere_pair = SpherePair::new(
        "one",
        &Url::parse("http://127.0.0.1:5001")?,
        &Url::parse("http://127.0.0.1:6667")?,
    )
    .await?;

    sphere_pair.start_gateway().await?;

    sphere_pair
        .spawn(move |mut client_sphere_context| async move {
            for value in ["one", "two", "three"] {
                client_sphere_context
                    .write(value, &ContentType::Text, value.as_bytes(), None)
                    .await?;
                client_sphere_context.save(None).await?;
            }

            client_sphere_context.sync(SyncRecovery::None).await?;

            Ok(())
        })
        .await?;

    Ok(())
}

#[tokio::test]
async fn gateway_can_sync_an_authorized_sphere_across_multiple_replicas() -> Result<()> {
    initialize_tracing(None);

    let mut sphere_pair = SpherePair::new(
        "one",
        &Url::parse("http://127.0.0.1:5001")?,
        &Url::parse("http://127.0.0.1:6667")?,
    )
    .await?;

    sphere_pair.start_gateway().await?;

    let (mut client_replica_workspace, _client_replica_temporary_directories) =
        temporary_workspace()?;
    let client_replica_key_name = "CLIENT_REPLICA_KEY";

    key_create(client_replica_key_name, &client_replica_workspace).await?;

    let gateway_url = sphere_pair.client.workspace.gateway_url().await?;

    sphere_pair
        .spawn(move |mut client_sphere_context| async move {
            for value in ["one", "two", "three"] {
                client_sphere_context
                    .write(value, &ContentType::Subtext, value.as_ref(), None)
                    .await?;
                SphereCursor::latest(client_sphere_context.clone())
                    .save(None)
                    .await?;
            }

            let client_replica_key_storage = client_replica_workspace.key_storage();
            let client_replica_key = client_replica_key_storage
                .require_key(client_replica_key_name)
                .await?;
            let client_replica_identity = Did(client_replica_key.get_did().await?);

            let client_replica_authorization = client_sphere_context
                .authorize("replica", &client_replica_identity)
                .await?;

            client_sphere_context.save(None).await?;

            client_sphere_context.sync(SyncRecovery::None).await?;

            sphere_join(
                client_replica_key_name,
                Some(client_replica_authorization.to_string()),
                &client_sphere_context.identity().await?,
                &gateway_url,
                None,
                &mut client_replica_workspace,
            )
            .await?;

            let mut client_replica_sphere_context =
                client_replica_workspace.sphere_context().await.unwrap();

            {
                let mut client_replica_sphere_context = client_replica_sphere_context.lock().await;
                client_replica_sphere_context
                    .configure_gateway_url(Some(&gateway_url))
                    .await?;
            }

            client_replica_sphere_context
                .sync(SyncRecovery::None)
                .await?;

            for value in ["one", "two", "three"] {
                let mut file = client_replica_sphere_context.read(value).await?.unwrap();
                let mut contents = String::new();
                file.contents.read_to_string(&mut contents).await?;
                assert_eq!(value, &contents);
            }
            Ok(())
        })
        .await?;

    Ok(())
}
