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
