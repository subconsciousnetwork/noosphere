#![cfg(not(target_arch = "wasm32"))]

use anyhow::{anyhow, Result};
use cid::Cid;
use libipld_cbor::DagCborCodec;
use std::{net::TcpListener, str::FromStr, time::Duration};
use tokio::io::AsyncReadExt;
use tokio_stream::StreamExt;
use ucan::{
    builder::UcanBuilder,
    crypto::{did::DidParser, KeyMaterial},
    store::UcanJwtStore,
    Ucan,
};
use ucan_key_support::ed25519::Ed25519KeyMaterial;
use url::Url;

use noosphere::key::KeyStorage;
use noosphere::sphere::SphereContext;
use noosphere_api::{
    data::{FetchParameters, FetchResponse, PushBody, PushResponse},
    route::Route,
};
use noosphere_cli::native::{
    commands::{
        auth::auth_add,
        key::key_create,
        serve::{
            gateway::{start_gateway, GatewayScope},
            tracing::initialize_tracing,
        },
        sphere::{sphere_create, sphere_join},
    },
    workspace::Workspace,
};
use noosphere_core::{
    authority::{generate_ed25519_key, Authorization, SUPPORTED_KEYS},
    data::{AddressIpld, ContentType, Did, MemoIpld},
    view::{Sphere, SphereMutation},
};
use noosphere_ns::{
    utils::{generate_capability, generate_fact},
    DHTKeyMaterial, Multiaddr, NSRecord, NameSystem, NameSystemBuilder,
};
use noosphere_storage::{BlockStore, MemoryStorage, SphereDb, Storage};

async fn create_bootstrap_node() -> Result<(
    Ed25519KeyMaterial,
    SphereDb<MemoryStorage>,
    NameSystem,
    Vec<Multiaddr>,
)> {
    let db = SphereDb::new(&MemoryStorage::default()).await?;
    let key = generate_ed25519_key();
    let node: NameSystem = NameSystemBuilder::default()
        .key_material(&key)
        .store(&db)
        .listening_port(0)
        .peer_dialing_interval(1)
        .build().await?;
    let address = node.p2p_addresses().await?.first().unwrap().to_owned();
    Ok((key, db, node, vec![address]))
}

/// Copies the authorization UCAN associated with `context` into the `dest_store`.
/// Sort of a hack to propagate the CID from local sphere to the name system's
/// store in order to validate.
async fn copy_authorization_ucan_token<K: DHTKeyMaterial + 'static, S1: Storage, S2: Storage>(
    context: &SphereContext<K, S1>,
    dest_store: &mut SphereDb<S2>,
) -> Result<()> {
    let sphere_db = context.db();
    let authorization = context.author().require_authorization()?;
    let authorization_cid = Cid::try_from(authorization)?;
    if let Some(token_str) = sphere_db.read_token(&authorization_cid).await? {
        let cid = dest_store.write_token(&token_str).await?;
        assert_eq!(cid, authorization_cid);
        Ok(())
    } else {
        Err(anyhow!("Authorization CID not found in store."))
    }
}

/// Creates a signed [NSRecord] for a phony sphere/from a phony owner.
/// Returns a tuple of the record itself, and the [Ucan] delegation.
async fn make_phony_ns_record(cid: &Cid) -> Result<(Did, NSRecord, Ucan)> {
    let owner_key = generate_ed25519_key();
    let owner_id = Did(owner_key.get_did().await?);
    let sphere_key = generate_ed25519_key();
    let sphere_id = Did(sphere_key.get_did().await?);

    let capability = generate_capability(&sphere_id);
    let delegation = UcanBuilder::default()
        .issued_by(&sphere_key)
        .for_audience(&owner_id)
        .with_lifetime(1000)
        .claiming_capability(&capability)
        .build()?
        .sign()
        .await?;

    let record: NSRecord = UcanBuilder::default()
        .issued_by(&owner_key)
        .for_audience(&sphere_id)
        .with_lifetime(1000)
        .claiming_capability(&capability)
        .with_fact(generate_fact(&cid.to_string()))
        .witnessed_by(&delegation)
        .build()?
        .sign()
        .await?
        .into();
    Ok((sphere_id, record, delegation))
}

async fn expect_address_in_ns<S: Storage>(
    ns: &NameSystem,
    db: &SphereDb<S>,
    identity: &String,
    expected_cid: &Cid,
) -> Result<()> {
    let record = {
        let mut i: u8 = 0;
        #[allow(unused_assignments)]
        let mut found_record: Option<NSRecord> = None;
        let did_identity = Did(identity.to_owned());
        loop {
            if let Some(record) = ns.get_record(&did_identity).await? {
                found_record = Some(record);
                break;
            } else {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            i += 1;
            if i > 100 {
                return Err(anyhow!("Waiting for result from NameSystem timed out"));
            }
        }
        found_record
    }
    .ok_or_else(|| anyhow!("No record found."))?;

    let mut did_parser = DidParser::new(SUPPORTED_KEYS);
    let _ = record.validate(db, &mut did_parser).await?;
    assert_eq!(record.identity(), identity);
    assert_eq!(record.link(), Some(expected_cid));
    Ok(())
}

#[tokio::test]
async fn gateway_tells_you_its_identity() {
    let (gateway_workspace, _gateway_temporary_directories) = Workspace::temporary().unwrap();
    let (client_workspace, _client_temporary_directories) = Workspace::temporary().unwrap();
    let (_bootstrap_key, _bootstrap_sphere, _bootstrap_node, bootstrap_addresses) =
        create_bootstrap_node().await.unwrap();

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
                None,
                &bootstrap_addresses,
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
    initialize_tracing();

    let (gateway_workspace, _gateway_temporary_directories) = Workspace::temporary().unwrap();
    let (client_workspace, _client_temporary_directories) = Workspace::temporary().unwrap();
    let (_bootstrap_key, _bootstrap_sphere, _bootstrap_node, bootstrap_addresses) =
        create_bootstrap_node().await.unwrap();

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
                None,
                &bootstrap_addresses,
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
    // initialize_tracing();

    let (gateway_workspace, _gateway_temporary_directories) = Workspace::temporary().unwrap();
    let (client_workspace, _client_temporary_directories) = Workspace::temporary().unwrap();
    let (_bootstrap_key, _bootstrap_sphere, _bootstrap_node, bootstrap_addresses) =
        create_bootstrap_node().await.unwrap();

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
                None,
                &bootstrap_addresses,
                None,
            )
            .await
            .unwrap()
        })
    };

    let client_sphere_context = client_workspace.sphere_context().await.unwrap();

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

        let sphere_cid = client_sphere_context
            .db()
            .require_version(&client_sphere_identity)
            .await
            .unwrap();

        let sphere = Sphere::at(&sphere_cid, client_sphere_context.db());
        let bundle = sphere.try_bundle_until_ancestor(None).await.unwrap();
        let client = client_sphere_context.client().await.unwrap();

        let push_result = client
            .push(&PushBody {
                sphere: client_sphere_identity.to_string(),
                base: None,
                tip: *sphere.cid(),
                blocks: bundle.clone(),
            })
            .await
            .unwrap();

        match push_result {
            PushResponse::Accepted { .. } => Ok(()),
            _ => Err(anyhow!("Unexpected push result")),
        }
        .unwrap();

        let block_stream = client_sphere_context.db().stream_links(&sphere_cid);

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
    // initialize_tracing();

    let (gateway_workspace, _gateway_temporary_directories) = Workspace::temporary().unwrap();
    let (client_workspace, _client_temporary_directories) = Workspace::temporary().unwrap();
    let (_bootstrap_key, _bootstrap_sphere, _bootstrap_node, bootstrap_addresses) =
        create_bootstrap_node().await.unwrap();

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
                None,
                &bootstrap_addresses,
                None,
            )
            .await
            .unwrap()
        })
    };

    let client_sphere_context = client_workspace.sphere_context().await.unwrap();

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

        let sphere_cid = client_sphere_context
            .db()
            .require_version(&client_sphere_identity)
            .await
            .unwrap();

        let mut sphere = Sphere::at(&sphere_cid, client_sphere_context.db());
        let bundle = sphere.try_bundle_until_ancestor(None).await.unwrap();

        let push_result = client
            .push(&PushBody {
                sphere: client_sphere_identity.to_string(),
                base: None,
                tip: *sphere.cid(),
                blocks: bundle.clone(),
            })
            .await
            .unwrap();

        match push_result {
            PushResponse::Accepted { .. } => Ok(()),
            _ => Err(anyhow!("Unexpected push result")),
        }
        .unwrap();

        let mut final_cid;

        for value in ["one", "two", "three"] {
            let memo = MemoIpld::for_body(client_sphere_context.db_mut(), vec![value])
                .await
                .unwrap();
            let memo_cid = client_sphere_context
                .db_mut()
                .save::<DagCborCodec, _>(&memo)
                .await
                .unwrap();

            let mut mutation =
                SphereMutation::new(&client_sphere_context.author().identity().await.unwrap());
            mutation.links_mut().set(&value.into(), &memo_cid);

            let mut revision = sphere.try_apply_mutation(&mutation).await.unwrap();
            final_cid = revision
                .try_sign(
                    &client_sphere_context.author().key,
                    client_sphere_context.author().authorization.as_ref(),
                )
                .await
                .unwrap();

            sphere = Sphere::at(&final_cid, client_sphere_context.db());
        }

        let bundle = sphere
            .try_bundle_until_ancestor(Some(&sphere_cid))
            .await
            .unwrap();

        let push_result = client
            .push(&PushBody {
                sphere: client_sphere_identity.to_string(),
                base: Some(sphere_cid),
                tip: *sphere.cid(),
                blocks: bundle,
            })
            .await
            .unwrap();

        match push_result {
            PushResponse::Accepted { .. } => Ok(()),
            _ => Err(anyhow!("Unexpected push result")),
        }
        .unwrap();

        let block_stream = client_sphere_context.db().stream_links(&sphere_cid);

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
async fn gateway_serves_sphere_revisions_to_a_client() {
    // initialize_tracing();

    let (gateway_workspace, _gateway_temporary_directories) = Workspace::temporary().unwrap();
    let (client_workspace, _client_temporary_directories) = Workspace::temporary().unwrap();
    let (_bootstrap_key, _bootstrap_sphere, _bootstrap_node, bootstrap_addresses) =
        create_bootstrap_node().await.unwrap();

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
                None,
                &bootstrap_addresses,
                None,
            )
            .await
            .unwrap()
        })
    };

    let client_sphere_context = client_workspace.sphere_context().await.unwrap();

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

        let sphere_cid = client_sphere_context
            .db()
            .require_version(&client_sphere_identity)
            .await
            .unwrap();

        let mut sphere = Sphere::at(&sphere_cid, client_sphere_context.db());

        for value in ["one", "two", "three"] {
            let memo = MemoIpld::for_body(client_sphere_context.db_mut(), vec![value])
                .await
                .unwrap();
            let memo_cid = client_sphere_context
                .db_mut()
                .save::<DagCborCodec, _>(&memo)
                .await
                .unwrap();

            let mut mutation =
                SphereMutation::new(&client_sphere_context.author().identity().await.unwrap());
            mutation.links_mut().set(&value.into(), &memo_cid);

            let mut revision = sphere.try_apply_mutation(&mutation).await.unwrap();

            let final_cid = revision
                .try_sign(
                    &client_sphere_context.author().key,
                    client_sphere_context.author().authorization.as_ref(),
                )
                .await
                .unwrap();

            sphere = Sphere::at(&final_cid, client_sphere_context.db());
        }

        let bundle = sphere.try_bundle_until_ancestor(None).await.unwrap();

        let push_result = client
            .push(&PushBody {
                sphere: client_sphere_identity.to_string(),
                base: None,
                tip: *sphere.cid(),
                blocks: bundle.clone(),
            })
            .await
            .unwrap();

        match push_result {
            PushResponse::Accepted { .. } => Ok(()),
            _ => Err(anyhow!("Unexpected push result")),
        }
        .unwrap();

        let fetch_result = client
            .fetch(&FetchParameters { since: None })
            .await
            .unwrap();

        match fetch_result {
            FetchResponse::NewChanges { .. } => Ok(()),
            _ => Err(anyhow!("Unexpected fetch result")),
        }
        .unwrap();

        server_task.abort();
        let _ = server_task.await;
    });

    client_task.await.unwrap();
}

#[tokio::test]
async fn gateway_can_sync_an_authorized_sphere_across_multiple_replicas() {
    // initialize_tracing();

    let (gateway_workspace, _gateway_temporary_directories) = Workspace::temporary().unwrap();
    let (client_workspace, _client_temporary_directories) = Workspace::temporary().unwrap();
    let (client_replica_workspace, _client_replica_temporary_directories) =
        Workspace::temporary().unwrap();
    let (_bootstrap_key, _bootstrap_sphere, _bootstrap_node, bootstrap_addresses) =
        create_bootstrap_node().await.unwrap();

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
                None,
                &bootstrap_addresses,
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

    let client_sphere_context = client_workspace.sphere_context().await.unwrap();
    let client_replica_sphere_context = client_replica_workspace.sphere_context().await.unwrap();

    let client_task = tokio::spawn(async move {
        let mut client_sphere_context = client_sphere_context.lock().await;
        let gateway_url: Url =
            format!("http://{}:{}", gateway_address.ip(), gateway_address.port())
                .parse()
                .unwrap();

        client_sphere_context
            .configure_gateway_url(Some(&gateway_url))
            .await
            .unwrap();

        for value in ["one", "two", "three"] {
            let mut fs = client_sphere_context.fs().await.unwrap();

            fs.write(
                value,
                &ContentType::Subtext.to_string(),
                value.as_ref(),
                None,
            )
            .await
            .unwrap();
            fs.save(None).await.unwrap();
        }

        client_sphere_context.sync().await.unwrap();

        let mut client_replica_sphere_context = client_replica_sphere_context.lock().await;
        client_replica_sphere_context
            .configure_gateway_url(Some(&gateway_url))
            .await
            .unwrap();

        client_replica_sphere_context.sync().await.unwrap();

        let fs = client_replica_sphere_context.fs().await.unwrap();

        for value in ["one", "two", "three"] {
            let mut file = fs.read(value).await.unwrap().unwrap();
            let mut contents = String::new();
            file.contents.read_to_string(&mut contents).await.unwrap();
            assert_eq!(value, &contents);
        }

        server_task.abort();
        let _ = server_task.await;
    });

    client_task.await.unwrap();
}

#[tokio::test]
async fn gateway_can_publish_spheres() {
    // initialize_tracing();

    let (gateway_workspace, _gateway_temporary_directories) = Workspace::temporary().unwrap();
    let (client_workspace, _client_temporary_directories) = Workspace::temporary().unwrap();
    let (_bootstrap_key, mut bootstrap_sphere, bootstrap_node, bootstrap_addresses) =
        create_bootstrap_node().await.unwrap();

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
                None,
                &bootstrap_addresses,
                Some(6667u16),
            )
            .await
            .unwrap()
        })
    };

    let client_sphere_context = client_workspace.sphere_context().await.unwrap();

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

        copy_authorization_ucan_token(&client_sphere_context, &mut bootstrap_sphere)
            .await
            .unwrap();

        client_sphere_context.publish().await.unwrap();

        let sphere = client_sphere_context.sphere().await.unwrap();
        let sphere_db = client_sphere_context.db();
        let sphere_cid = sphere.cid();

        expect_address_in_ns(
            &bootstrap_node,
            sphere_db,
            &client_sphere_identity,
            &sphere_cid,
        )
        .await
        .unwrap();

        server_task.abort();
        let _ = server_task.await;
    });

    client_task.await.unwrap();
}

/*
#[tokio::test]
async fn gateway_can_update_names() {
    // initialize_tracing();

    let (gateway_workspace, _gateway_temporary_directories) = Workspace::temporary().unwrap();
    let (client_workspace, _client_temporary_directories) = Workspace::temporary().unwrap();
    let (_bootstrap_key, mut bootstrap_sphere, bootstrap_node, bootstrap_addresses) =
        create_bootstrap_node().await.unwrap();

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
                None,
                &bootstrap_addresses,
                None,
            )
            .await
            .unwrap()
        })
    };

    let client_sphere_context = client_workspace.sphere_context().await.unwrap();

    let other_sphere_pet_name = String::from("freyja");
    let other_sphere_cid =
        Cid::from_str("bafy2bzacec4p5h37mjk2n6qi6zukwyzkruebvwdzqpdxzutu4sgoiuhqwne72").unwrap();
    let (other_sphere_id, other_sphere_record, other_sphere_delegate) =
        make_phony_ns_record(&other_sphere_cid).await.unwrap();

    {
        // Push a NSRecord to bootstrap node, and the corresponding delegation
        // token to bootstrap and clients, in order to test resolving
        // of pet names.
        let mut client_sphere_context = client_sphere_context.lock().await;

        let delegate_str = other_sphere_delegate.encode().unwrap();
        bootstrap_sphere.write_token(&delegate_str).await.unwrap();
        client_sphere_context
            .db_mut()
            .write_token(&delegate_str)
            .await
            .unwrap();

        if let Err(e) = bootstrap_node.put_record(other_sphere_record.clone()).await {
            // This is an expected error, should be a better way of handling this.
            println!(
                "Bootstrap node did not propagate record, to be expected with a single node. {:#?}",
                e
            );
        }
    }

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

        let mut sphere = {
            let sphere_cid = client_sphere_context
                .db()
                .require_version(&client_sphere_identity)
                .await
                .unwrap();
            let sphere = Sphere::at(&sphere_cid, client_sphere_context.db());

            let author = client_sphere_context.author();
            let address = AddressIpld {
                identity: other_sphere_id.clone(),
                last_known_record: None,
            };

            let mut mutation = SphereMutation::new(&author.identity().await.unwrap());
            mutation.names_mut().set(&other_sphere_pet_name, &address);

            let mut revision = sphere.try_apply_mutation(&mutation).await.unwrap();

            let cid = revision
                .try_sign(&author.key, author.authorization.as_ref())
                .await
                .unwrap();

            Sphere::at(&cid, client_sphere_context.db())
        };
        let bundle = sphere.try_bundle_until_ancestor(None).await.unwrap();

        let _ = client
            .push(&PushBody {
                sphere: client_sphere_identity.to_string(),
                base: None,
                tip: *sphere.cid(),
                blocks: bundle.clone(),
            })
            .await
            .unwrap();

        let sphere_cid = sphere.cid().to_owned();
        println!("Sphere Cid starting: {}", sphere_cid);
        loop {
            client_sphere_context.sync().await.unwrap();
            sphere = client_sphere_context.sphere().await.unwrap();
            println!("next Cid : {}", sphere_cid);
            if sphere.cid() != &sphere_cid {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        let sphere = client_sphere_context.sphere().await.unwrap();
        let names = sphere.try_get_names().await.unwrap();
        let address = names.get(&other_sphere_pet_name).await.unwrap().unwrap();
        let record = NSRecord::from_str(address.last_known_record.as_ref().unwrap()).unwrap();
        assert_eq!(record.link().unwrap(), &other_sphere_cid);
        assert_eq!(address.identity, other_sphere_id);

        server_task.abort();
        let _ = server_task.await;
    });

    client_task.await.unwrap();
}
*/