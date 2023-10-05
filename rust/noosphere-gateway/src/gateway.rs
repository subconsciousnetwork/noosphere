use anyhow::Result;
use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderValue, Method};
use axum::routing::{get, put};
use axum::{Extension, Router, Server};
use noosphere_core::context::HasMutableSphereContext;
use noosphere_core::data::Did;
use noosphere_ipfs::KuboClient;
use noosphere_storage::Storage;
use std::net::TcpListener;
use std::path::Path;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use url::Url;

use noosphere_core::api::{v0alpha1, v0alpha2};

use crate::worker::start_iroh_syndication;
use crate::{
    handlers,
    worker::{start_cleanup, start_name_system, NameSystemConfiguration, NameSystemConnectionType},
};

use noosphere_core::tracing::initialize_tracing;

const DEFAULT_BODY_LENGTH_LIMIT: usize = 100 /* MB */ * 1000 * 1000;

/// A [GatewayScope] describes the pairing of a gateway and its designated user
/// via their spheres' respective [Did]s
#[derive(Clone, Debug)]
pub struct GatewayScope {
    /// Identity of gateway sphere.
    pub identity: Did,
    /// Identity of a managed sphere that is being reflected by gateway sphere.
    pub counterpart: Did,
}

pub use iroh::rpc_protocol::DocTicket;

/// Start a Noosphere Gateway
pub async fn start_gateway<C, S>(
    listener: TcpListener,
    gateway_scope: GatewayScope,
    sphere_context: C,
    ipfs_api: Url,
    iroh_ticket: DocTicket,
    sphere_path: impl AsRef<Path>,
    name_resolver_api: Url,
    cors_origin: Option<Url>,
) -> Result<()>
where
    C: HasMutableSphereContext<S> + 'static,
    S: Storage + 'static,
{
    initialize_tracing(None);

    let gateway_key_did = {
        let sphere_context = sphere_context.sphere_context().await?;
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

    let (syndication_tx, syndication_task) =
        start_iroh_syndication::<C, S>(sphere_path, iroh_ticket);
    let ipfs_client = KuboClient::new(&ipfs_api)?;
    let (name_system_tx, name_system_task) = start_name_system::<C, S>(
        NameSystemConfiguration {
            connection_type: NameSystemConnectionType::Remote(name_resolver_api),
            ipfs_api,
        },
        vec![sphere_context.clone()],
    );
    let (cleanup_tx, cleanup_task) = start_cleanup::<C, S>(sphere_context.clone());

    let app = Router::new()
        .route(
            &v0alpha1::Route::Did.to_string(),
            get(handlers::v0alpha1::did_route),
        )
        .route(
            &v0alpha1::Route::Replicate(None).to_string(),
            get(handlers::v0alpha1::replicate_route::<C, S>),
        )
        .route(
            &v0alpha1::Route::Identify.to_string(),
            get(handlers::v0alpha1::identify_route::<C, S>),
        )
        .route(
            &v0alpha1::Route::Push.to_string(),
            #[allow(deprecated)]
            put(handlers::v0alpha1::push_route::<C, S>),
        )
        .route(
            &v0alpha2::Route::Push.to_string(),
            put(handlers::v0alpha2::push_route::<C, S>),
        )
        .route(
            &v0alpha1::Route::Fetch.to_string(),
            get(handlers::v0alpha1::fetch_route::<C, S>),
        )
        .layer(Extension(sphere_context.clone()))
        .layer(Extension(gateway_scope.clone()))
        .layer(Extension(ipfs_client))
        .layer(Extension(gateway_key_did))
        .layer(Extension(syndication_tx))
        .layer(Extension(name_system_tx))
        .layer(Extension(cleanup_tx))
        .layer(DefaultBodyLimit::max(DEFAULT_BODY_LENGTH_LIMIT))
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    info!(
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
    cleanup_task.abort();

    Ok(())
}
