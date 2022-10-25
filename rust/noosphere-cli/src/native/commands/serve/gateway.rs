use anyhow::Result;
use axum::http::{HeaderValue, Method};
use axum::routing::{get, put};
use axum::{Extension, Router, Server};
use std::net::TcpListener;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use ucan::crypto::KeyMaterial;

use noosphere::authority::{Authorization, SUPPORTED_KEYS};
use noosphere_api::route::Route as GatewayRoute;
use noosphere_storage::{db::SphereDb, native::NativeStore};
use ucan::crypto::did::DidParser;
use url::Url;

use crate::native::commands::serve::route::{did_route, fetch_route, identify_route, push_route};
use crate::native::commands::serve::tracing::initialize_tracing;

#[derive(Clone, Debug)]
pub struct GatewayScope {
    pub identity: String,
    pub counterpart: String,
}

pub async fn start_gateway<K>(
    listener: TcpListener,
    gateway_key: K,
    gateway_scope: GatewayScope,
    gateway_authorization: Authorization,
    gateway_db: SphereDb<NativeStore>,
    cors_origin: Option<Url>,
) -> Result<()>
where
    K: KeyMaterial + 'static,
{
    initialize_tracing();
    let did_parser = DidParser::new(SUPPORTED_KEYS);

    let mut cors = CorsLayer::new();

    if let Some(cors_origin) = cors_origin {
        cors = cors
            .allow_origin(
                cors_origin
                    .origin()
                    .unicode_serialization()
                    .as_str()
                    .parse::<HeaderValue>()?,
            )
            .allow_headers(Any)
            .allow_methods(vec![
                Method::GET,
                Method::POST,
                Method::PATCH,
                Method::PUT,
                Method::DELETE,
            ]);
    }

    let app = Router::new()
        .route(&GatewayRoute::Did.to_string(), get(did_route::<K>))
        .route(
            &GatewayRoute::Identify.to_string(),
            get(identify_route::<K>),
        )
        .route(&GatewayRoute::Push.to_string(), put(push_route::<K>))
        .route(&GatewayRoute::Fetch.to_string(), get(fetch_route))
        .layer(Extension(gateway_db))
        .layer(Extension(gateway_scope.clone()))
        .layer(Extension(gateway_authorization))
        .layer(Extension(Arc::new(Mutex::new(did_parser))))
        .layer(Extension(Arc::new(gateway_key)))
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    println!(
        r#"A geist is summoned to manage local sphere {}

It has bound a gateway to {:?}
It awaits updates from sphere {}..."#,
        gateway_scope.identity,
        listener
            .local_addr()
            .expect("Unexpected missing listener address"),
        gateway_scope.counterpart
    );

    Server::from_tcp(listener)?
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use anyhow::anyhow;
    use std::net::TcpListener;

    use noosphere::{
        authority::SUPPORTED_KEYS,
        data::MemoIpld,
        view::{Sphere, SphereMutation},
    };
    use noosphere_api::{
        client::Client,
        data::{FetchParameters, FetchResponse, PushBody, PushResponse},
    };

    use noosphere_storage::interface::BlockStore;

    use libipld_cbor::DagCborCodec;
    use tokio_stream::StreamExt;
    use ucan::crypto::{did::DidParser, KeyMaterial};
    use url::Url;

    use crate::native::{
        commands::{key::key_create, sphere::sphere_create},
        workspace::Workspace,
    };

    use super::{start_gateway, GatewayScope};

    #[tokio::test]
    async fn it_can_be_identified_by_the_client_of_its_owner() {
        // initialize_tracing();

        let gateway_workspace = Workspace::temporary().unwrap();
        let client_workspace = Workspace::temporary().unwrap();

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

        let client_key = client_workspace.get_local_key().await.unwrap();
        let gateway_key = gateway_workspace.get_local_key().await.unwrap();
        let gateway_identity = gateway_key.get_did().await.unwrap();
        let gateway_authority = gateway_workspace.get_local_authorization().await.unwrap();

        let gateway_sphere_identity = gateway_workspace.get_local_identity().await.unwrap();
        let client_sphere_identity = client_workspace.get_local_identity().await.unwrap();

        let gateway_db = gateway_workspace.get_local_db().await.unwrap();
        let client_db = client_workspace.get_local_db().await.unwrap();

        let server_task = tokio::spawn(async move {
            start_gateway(
                listener,
                gateway_key,
                GatewayScope {
                    identity: gateway_sphere_identity,
                    counterpart: client_sphere_identity,
                },
                gateway_authority,
                gateway_db,
                None,
            )
            .await
            .unwrap()
        });

        let client_sphere_identity = client_workspace.get_local_identity().await.unwrap();
        let client_authorization = client_workspace.get_local_authorization().await.unwrap();

        let client_task = tokio::spawn(async move {
            let api_base: Url =
                format!("http://{}:{}", gateway_address.ip(), gateway_address.port())
                    .parse()
                    .unwrap();
            let mut did_parser = DidParser::new(SUPPORTED_KEYS);

            let client = Client::identify(
                &client_sphere_identity,
                &api_base,
                &client_key,
                &client_authorization,
                &mut did_parser,
                client_db,
            )
            .await
            .unwrap();

            assert_eq!(client.session.gateway_identity, gateway_identity);

            server_task.abort();
            let _ = server_task.await;
        });

        client_task.await.unwrap();
    }

    #[tokio::test]
    async fn it_can_receive_a_newly_initialized_sphere_from_the_client() {
        // initialize_tracing();

        let gateway_workspace = Workspace::temporary().unwrap();
        let client_workspace = Workspace::temporary().unwrap();

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

        let client_key = client_workspace.get_local_key().await.unwrap();
        let gateway_key = gateway_workspace.get_local_key().await.unwrap();
        let gateway_authority = gateway_workspace.get_local_authorization().await.unwrap();

        let gateway_sphere_identity = gateway_workspace.get_local_identity().await.unwrap();
        let client_sphere_identity = client_workspace.get_local_identity().await.unwrap();

        let gateway_db = gateway_workspace.get_local_db().await.unwrap();
        let client_db = client_workspace.get_local_db().await.unwrap();

        let server_task = {
            let gateway_db = gateway_db.clone();
            tokio::spawn(async move {
                start_gateway(
                    listener,
                    gateway_key,
                    GatewayScope {
                        identity: gateway_sphere_identity,
                        counterpart: client_sphere_identity,
                    },
                    gateway_authority,
                    gateway_db,
                    None,
                )
                .await
                .unwrap()
            })
        };

        let client_sphere_identity = client_workspace.get_local_identity().await.unwrap();
        let client_authorization = client_workspace.get_local_authorization().await.unwrap();

        let client_task = tokio::spawn(async move {
            let api_base: Url =
                format!("http://{}:{}", gateway_address.ip(), gateway_address.port())
                    .parse()
                    .unwrap();
            let mut did_parser = DidParser::new(SUPPORTED_KEYS);

            let client = Client::identify(
                &client_sphere_identity,
                &api_base,
                &client_key,
                &client_authorization,
                &mut did_parser,
                client_db.clone(),
            )
            .await
            .unwrap();

            let sphere_cid = client_db
                .get_version(&client_sphere_identity)
                .await
                .unwrap()
                .unwrap();
            let sphere = Sphere::at(&sphere_cid, &client_db);
            let bundle = sphere.try_bundle_until_ancestor(None).await.unwrap();

            let push_result = client
                .push(&PushBody {
                    sphere: client_sphere_identity,
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

            let block_stream = client_db.stream_links(&sphere_cid);

            tokio::pin!(block_stream);

            while let Some(cid) = block_stream.try_next().await.unwrap() {
                assert!(gateway_db.get_block(&cid).await.unwrap().is_some());
            }

            server_task.abort();

            let _ = server_task.await;
        });

        client_task.await.unwrap();
    }

    #[tokio::test]
    async fn it_can_update_an_existing_sphere_with_changes_from_the_client() {
        // initialize_tracing();

        let gateway_workspace = Workspace::temporary().unwrap();
        let client_workspace = Workspace::temporary().unwrap();

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

        let client_key = client_workspace.get_local_key().await.unwrap();
        let gateway_key = gateway_workspace.get_local_key().await.unwrap();
        let gateway_identity = gateway_key.get_did().await.unwrap();
        let gateway_authorization = gateway_workspace.get_local_authorization().await.unwrap();

        let gateway_sphere_identity = gateway_workspace.get_local_identity().await.unwrap();
        let client_sphere_identity = client_workspace.get_local_identity().await.unwrap();

        let gateway_db = gateway_workspace.get_local_db().await.unwrap();
        let mut client_db = client_workspace.get_local_db().await.unwrap();

        let server_task = {
            let gateway_db = gateway_db.clone();
            tokio::spawn(async move {
                start_gateway(
                    listener,
                    gateway_key,
                    GatewayScope {
                        identity: gateway_sphere_identity,
                        counterpart: client_sphere_identity,
                    },
                    gateway_authorization,
                    gateway_db,
                    None,
                )
                .await
                .unwrap()
            })
        };

        let client_sphere_identity = client_workspace.get_local_identity().await.unwrap();
        let client_authorization = client_workspace.get_local_authorization().await.unwrap();
        let client_did = client_key.get_did().await.unwrap();

        let client_task = tokio::spawn(async move {
            let api_base: Url =
                format!("http://{}:{}", gateway_address.ip(), gateway_address.port())
                    .parse()
                    .unwrap();
            let mut did_parser = DidParser::new(SUPPORTED_KEYS);

            let client = Client::identify(
                &client_sphere_identity,
                &api_base,
                &client_key,
                &client_authorization,
                &mut did_parser,
                client_db.clone(),
            )
            .await
            .unwrap();

            assert_eq!(client.session.gateway_identity, gateway_identity);

            let sphere_cid = client_db
                .get_version(&client_sphere_identity)
                .await
                .unwrap()
                .unwrap();
            let mut sphere = Sphere::at(&sphere_cid, &client_db);
            let bundle = sphere.try_bundle_until_ancestor(None).await.unwrap();

            let push_result = client
                .push(&PushBody {
                    sphere: client_sphere_identity.clone(),
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

            for value in ["one", "two", "three"] {
                let memo = MemoIpld::for_body(&mut client_db, vec![value])
                    .await
                    .unwrap();
                let memo_cid = client_db.save::<DagCborCodec, _>(&memo).await.unwrap();

                let mut mutation = SphereMutation::new(&client_did);
                mutation.links_mut().set(&value.into(), &memo_cid);

                let mut revision = sphere.try_apply_mutation(&mutation).await.unwrap();
                let final_cid = revision
                    .try_sign(&client_key, Some(&client_authorization))
                    .await
                    .unwrap();

                sphere = Sphere::at(&final_cid, &client_db);
            }

            let bundle = sphere
                .try_bundle_until_ancestor(Some(&sphere_cid))
                .await
                .unwrap();

            let push_result = client
                .push(&PushBody {
                    sphere: client_sphere_identity,
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

            let block_stream = client_db.stream_links(&sphere_cid);

            tokio::pin!(block_stream);

            while let Some(cid) = block_stream.try_next().await.unwrap() {
                assert!(gateway_db.get_block(&cid).await.unwrap().is_some());
            }

            server_task.abort();
            let _ = server_task.await;
        });

        client_task.await.unwrap();
    }

    #[tokio::test]
    async fn it_can_serve_sphere_revisions_to_a_client() {
        // initialize_tracing();

        let gateway_workspace = Workspace::temporary().unwrap();
        let client_workspace = Workspace::temporary().unwrap();

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

        let client_key = client_workspace.get_local_key().await.unwrap();
        let gateway_key = gateway_workspace.get_local_key().await.unwrap();
        let gateway_identity = gateway_key.get_did().await.unwrap();
        let gateway_authority = gateway_workspace.get_local_authorization().await.unwrap();

        let gateway_sphere_identity = gateway_workspace.get_local_identity().await.unwrap();
        let client_sphere_identity = client_workspace.get_local_identity().await.unwrap();

        let gateway_db = gateway_workspace.get_local_db().await.unwrap();
        let mut client_db = client_workspace.get_local_db().await.unwrap();

        let server_task = {
            let gateway_db = gateway_db.clone();
            tokio::spawn(async move {
                start_gateway(
                    listener,
                    gateway_key,
                    GatewayScope {
                        identity: gateway_sphere_identity,
                        counterpart: client_sphere_identity,
                    },
                    gateway_authority,
                    gateway_db,
                    None,
                )
                .await
                .unwrap()
            })
        };

        let client_sphere_identity = client_workspace.get_local_identity().await.unwrap();
        let client_authorization = client_workspace.get_local_authorization().await.unwrap();
        let client_did = client_key.get_did().await.unwrap();

        let client_task = tokio::spawn(async move {
            let api_base: Url =
                format!("http://{}:{}", gateway_address.ip(), gateway_address.port())
                    .parse()
                    .unwrap();
            let mut did_parser = DidParser::new(SUPPORTED_KEYS);

            let client = Client::identify(
                &client_sphere_identity,
                &api_base,
                &client_key,
                &client_authorization,
                &mut did_parser,
                client_db.clone(),
            )
            .await
            .unwrap();

            assert_eq!(client.session.gateway_identity, gateway_identity);

            let sphere_cid = client_db
                .get_version(&client_sphere_identity)
                .await
                .unwrap()
                .unwrap();
            let mut sphere = Sphere::at(&sphere_cid, &client_db);

            let mut final_cid = sphere_cid;

            for value in ["one", "two", "three"] {
                let memo = MemoIpld::for_body(&mut client_db, vec![value])
                    .await
                    .unwrap();
                let memo_cid = client_db.save::<DagCborCodec, _>(&memo).await.unwrap();

                let mut mutation = SphereMutation::new(&client_did);
                mutation.links_mut().set(&value.into(), &memo_cid);

                let mut revision = sphere.try_apply_mutation(&mutation).await.unwrap();

                final_cid = revision
                    .try_sign(&client_key, Some(&client_authorization))
                    .await
                    .unwrap();

                sphere = Sphere::at(&final_cid, &client_db);
            }

            let sphere = Sphere::at(&final_cid, &client_db);
            let bundle = sphere.try_bundle_until_ancestor(None).await.unwrap();

            let push_result = client
                .push(&PushBody {
                    sphere: client_sphere_identity,
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
}
