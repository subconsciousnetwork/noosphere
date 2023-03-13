use anyhow::Result;
use axum::http::{HeaderValue, Method};
use axum::routing::{get, put};
use axum::{Extension, Router, Server};
use noosphere_core::data::Did;
use noosphere_sphere::HasMutableSphereContext;
use std::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use ucan::crypto::KeyMaterial;
use url::Url;

use noosphere_api::route::Route as GatewayRoute;
use noosphere_storage::Storage;

use crate::nns::{start_name_system, NameSystemConfiguration};
use crate::{
    ipfs::start_ipfs_syndication,
    route::{did_route, fetch_route, identify_route, push_route},
};

use noosphere_core::tracing::initialize_tracing;

#[derive(Clone, Debug)]
pub struct GatewayScope {
    pub identity: Did,
    pub counterpart: Did,
}

pub async fn start_gateway<H, K, S>(
    listener: TcpListener,
    gateway_scope: GatewayScope,
    sphere_context: H,
    ipfs_api: Url,
    name_resolver_api: Url,
    cors_origin: Option<Url>,
) -> Result<()>
where
    H: HasMutableSphereContext<K, S> + 'static,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    initialize_tracing();

    let gateway_key_did = sphere_context.identity().await?;

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

    let (syndication_tx, syndication_task) = start_ipfs_syndication::<H, K, S>(ipfs_api);
    let (name_system_tx, name_system_task) = start_name_system::<H, K, S>(
        NameSystemConfiguration::Remote(name_resolver_api),
        vec![sphere_context.clone()],
    );

    let app = Router::new()
        .route(&GatewayRoute::Did.to_string(), get(did_route::<K>))
        .route(
            &GatewayRoute::Identify.to_string(),
            get(identify_route::<H, K, S>),
        )
        .route(&GatewayRoute::Push.to_string(), put(push_route::<H, K, S>))
        .route(
            &GatewayRoute::Fetch.to_string(),
            get(fetch_route::<H, K, S>),
        )
        .layer(Extension(sphere_context.clone()))
        .layer(Extension(gateway_scope.clone()))
        .layer(Extension(gateway_key_did))
        .layer(Extension(syndication_tx))
        .layer(Extension(name_system_tx))
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
    name_system_task.abort();

    Ok(())
}
