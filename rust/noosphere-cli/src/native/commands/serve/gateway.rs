use anyhow::Result;
use axum::http::{HeaderValue, Method};
use axum::routing::{get, put};
use axum::{Extension, Router, Server};
use noosphere::sphere::SphereContext;
use noosphere_core::data::Did;
use std::net::TcpListener;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use ucan::crypto::KeyMaterial;
use url::Url;

use noosphere_api::route::Route as GatewayRoute;
use noosphere_storage::native::NativeStore;

use crate::native::commands::serve::route::{did_route, fetch_route, identify_route, push_route};
use crate::native::commands::serve::tracing::initialize_tracing;

#[derive(Clone, Debug)]
pub struct GatewayScope {
    pub identity: Did,
    pub counterpart: Did,
}

pub async fn start_gateway<K>(
    listener: TcpListener,
    gateway_scope: GatewayScope,
    sphere_context: Arc<Mutex<SphereContext<K, NativeStore>>>,
    cors_origin: Option<Url>,
) -> Result<()>
where
    K: KeyMaterial + Clone + 'static,
{
    initialize_tracing();

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
        .route(&GatewayRoute::Fetch.to_string(), get(fetch_route::<K>))
        .layer(Extension(sphere_context))
        .layer(Extension(gateway_scope.clone()))
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
    use noosphere_storage::interface::BlockStore;
    use std::net::TcpListener;
    use tokio_stream::StreamExt;

    use noosphere_api::data::{FetchParameters, FetchResponse, PushBody, PushResponse};
    use noosphere_core::{
        data::MemoIpld,
        view::{Sphere, SphereMutation},
    };

    use libipld_cbor::DagCborCodec;
    use ucan::crypto::KeyMaterial;

    use crate::native::{
        commands::{key::key_create, serve::tracing::initialize_tracing, sphere::sphere_create},
        workspace::Workspace,
    };

    use super::{start_gateway, GatewayScope};

    #[tokio::test]
    async fn it_can_be_identified_by_the_client_of_its_owner() {
        initialize_tracing();

        let (gateway_workspace, gateway_temporary_directories) = Workspace::temporary().unwrap();
        let (client_workspace, client_temporary_directories) = Workspace::temporary().unwrap();

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

        drop(gateway_temporary_directories);
        drop(client_temporary_directories);
    }

    #[tokio::test]
    async fn it_can_receive_a_newly_initialized_sphere_from_the_client() {
        // initialize_tracing();

        let (gateway_workspace, gateway_temporary_directories) = Workspace::temporary().unwrap();
        let (client_workspace, client_temporary_directories) = Workspace::temporary().unwrap();

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

        drop(gateway_temporary_directories);
        drop(client_temporary_directories);
    }

    #[tokio::test]
    async fn it_can_update_an_existing_sphere_with_changes_from_the_client() {
        // initialize_tracing();

        let (gateway_workspace, gateway_temporary_directories) = Workspace::temporary().unwrap();
        let (client_workspace, client_temporary_directories) = Workspace::temporary().unwrap();

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

        drop(gateway_temporary_directories);
        drop(client_temporary_directories);
    }

    #[tokio::test]
    async fn it_can_serve_sphere_revisions_to_a_client() {
        // initialize_tracing();

        let (gateway_workspace, gateway_temporary_directories) = Workspace::temporary().unwrap();
        let (client_workspace, client_temporary_directories) = Workspace::temporary().unwrap();

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

            let mut final_cid = sphere_cid;

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

            let sphere = Sphere::at(&final_cid, client_sphere_context.db());
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

        drop(gateway_temporary_directories);
        drop(client_temporary_directories);
    }
}
