use anyhow::Result;
use axum::http::{HeaderValue, Method};
use axum::routing::{get, post, put};
use axum::{Extension, Router, Server};
use noosphere::sphere::SphereContext;
use noosphere_core::data::Did;
use std::net::TcpListener;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use url::Url;

use noosphere_api::route::Route as GatewayRoute;
use noosphere_ns::{DHTKeyMaterial, Multiaddr};
use noosphere_storage::NativeStorage;

use crate::native::commands::serve::{
    ipfs::start_ipfs_syndication,
    name_system::start_ns_service,
    route::{did_route, fetch_route, identify_route, publish_route, push_route},
    tracing::initialize_tracing,
};

#[derive(Clone, Debug)]
pub struct GatewayScope {
    pub identity: Did,
    pub counterpart: Did,
}

/// Starts the gateway server with given configuration.
pub async fn start_gateway<K>(
    listener: TcpListener,
    gateway_scope: GatewayScope,
    sphere_context: Arc<Mutex<SphereContext<K, NativeStorage>>>,
    ipfs_api: Url,
    cors_origin: Option<Url>,
    bootstrap_peers: &[Multiaddr],
    ns_port: Option<u16>,
) -> Result<()>
where
    K: DHTKeyMaterial + 'static,
{
    initialize_tracing();

    let gateway_key_did = {
        let sphere_context = sphere_context.lock().await;
        sphere_context.author().identity().await?
    };

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

    let (syndication_tx, syndication_task) = start_ipfs_syndication::<K, NativeStorage>(ipfs_api);
    let (ns_tx, ns_task) = if ns_port.is_some() {
        let (ns_tx, ns_task) =
            start_ns_service::<K, NativeStorage>(sphere_context.clone(), bootstrap_peers, ns_port);
        (Some(ns_tx), Some(ns_task))
    } else {
        (None, None)
    };

    let app = Router::new()
        .route(&GatewayRoute::Did.to_string(), get(did_route::<K>))
        .route(
            &GatewayRoute::Identify.to_string(),
            get(identify_route::<K>),
        )
        .route(&GatewayRoute::Push.to_string(), put(push_route::<K>))
        .route(&GatewayRoute::Fetch.to_string(), get(fetch_route::<K>))
        .route(&GatewayRoute::Publish.to_string(), post(publish_route::<K>))
        .layer(Extension(sphere_context.clone()))
        .layer(Extension(gateway_scope.clone()))
        .layer(Extension(gateway_key_did))
        .layer(Extension(syndication_tx))
        .layer(Extension(ns_tx))
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

    syndication_task.abort();
    if let Some(ns_task) = ns_task {
        ns_task.abort();
    }

    Ok(())
}
