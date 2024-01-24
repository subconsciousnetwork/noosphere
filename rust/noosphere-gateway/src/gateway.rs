use crate::handlers;
use crate::GatewayManager;
use anyhow::Result;
use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderValue, Method};
use axum::routing::{get, put};
use axum::{serve, Extension, Router};
use noosphere_core::api::{v0alpha1, v0alpha2};
use noosphere_core::context::HasMutableSphereContext;
use noosphere_ipfs::KuboClient;
use noosphere_storage::Storage;
use std::net::TcpListener;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

#[cfg(feature = "observability")]
use axum_tracing_opentelemetry::middleware::{OtelAxumLayer, OtelInResponseLayer};

const DEFAULT_BODY_LENGTH_LIMIT: usize = 100 /* MB */ * 1000 * 1000;

/// Represents a Noosphere gateway server.
pub struct Gateway {
    router: Router,
}

impl Gateway {
    /// Create a new Noosphere `Gateway`, initializing worker threads
    /// and router configurations. Use [Gateway::start] to start the server.
    pub fn new<M, C, S>(manager: M) -> Result<Self>
    where
        M: GatewayManager<C, S> + 'static,
        C: HasMutableSphereContext<S> + 'static,
        S: Storage + 'static,
    {
        let mut cors = CorsLayer::new();

        if let Some(cors_origin) = manager.cors_origin() {
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

        let job_runner_client = manager.job_client();
        let ipfs_client = KuboClient::new(&manager.ipfs_api_url())?;

        let router = Router::new()
            .route("/healthz", get(|| async {}))
            .route(
                &v0alpha1::Route::Did.to_string(),
                get(handlers::v0alpha1::did_route::<C, S>),
            )
            .route(
                &v0alpha1::Route::Replicate(None).to_string(),
                get(handlers::v0alpha1::replicate_route::<M, C, S>),
            )
            .route(
                &v0alpha1::Route::Identify.to_string(),
                get(handlers::v0alpha1::identify_route::<M, C, S>),
            )
            .route(
                &v0alpha1::Route::Push.to_string(),
                #[allow(deprecated)]
                put(handlers::v0alpha1::push_route::<M, C, S>),
            )
            .route(
                &v0alpha2::Route::Push.to_string(),
                put(handlers::v0alpha2::push_route::<M, C, S>),
            )
            .route(
                &v0alpha1::Route::Fetch.to_string(),
                get(handlers::v0alpha1::fetch_route::<M, C, S>),
            )
            .layer(Extension(ipfs_client))
            .layer(Extension(job_runner_client))
            .layer(DefaultBodyLimit::max(DEFAULT_BODY_LENGTH_LIMIT))
            .layer(cors);

        #[cfg(feature = "observability")]
        let router = {
            router
                .layer(OtelInResponseLayer) // include trace context in response
                .layer(OtelAxumLayer::default()) // initialize otel trace on incoming request
        };

        let router = router
            .layer(TraceLayer::new_for_http())
            .with_state(Arc::new(manager));

        Ok(Self { router })
    }

    /// Start the gateway server with `listener`, consuming the [Gateway]
    /// object until the process terminates or has an unrecoverable error.
    pub async fn start(self, listener: TcpListener) -> Result<()> {
        // Listener must be set to nonblocking
        // https://docs.rs/tokio/latest/tokio/net/struct.TcpListener.html#method.from_std
        listener.set_nonblocking(true)?;
        let tokio_listener = tokio::net::TcpListener::from_std(listener)?;
        serve(tokio_listener, self.router.into_make_service()).await?;
        Ok(())
    }
}
