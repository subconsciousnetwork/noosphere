use crate::GatewayManager;
use crate::{
    handlers,
    worker::{
        start_cleanup, start_ipfs_syndication, start_name_system, NameSystemConfiguration,
        NameSystemConnectionType,
    },
};
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
use tokio::task::JoinHandle;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use url::Url;

#[cfg(feature = "observability")]
use axum_tracing_opentelemetry::middleware::{OtelAxumLayer, OtelInResponseLayer};

const DEFAULT_BODY_LENGTH_LIMIT: usize = 100 /* MB */ * 1000 * 1000;

type WorkerHandles = Vec<JoinHandle<Result<()>>>;

/// Represents a Noosphere gateway server.
pub struct Gateway {
    router: Router,
    worker_handles: WorkerHandles,
}

impl Gateway {
    /// Create a new Noosphere `Gateway`, initializing worker threads
    /// and router configurations. Use [Gateway::start] to start the server.
    pub fn new<M, C, S>(
        manager: M,
        ipfs_api: Url,
        name_resolver_api: Url,
        cors_origin: Option<Url>,
    ) -> Result<Self>
    where
        M: GatewayManager<C, S> + 'static,
        C: HasMutableSphereContext<S> + 'static,
        S: Storage + 'static,
    {
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

        let ipfs_client = KuboClient::new(&ipfs_api)?;

        let (syndication_tx, syndication_task) =
            start_ipfs_syndication::<M, C, S>(ipfs_api.clone(), manager.clone());
        let (name_system_tx, name_system_task) = start_name_system::<M, C, S>(
            NameSystemConfiguration {
                connection_type: NameSystemConnectionType::Remote(name_resolver_api),
                ipfs_api,
            },
            manager.clone(),
        );
        let (cleanup_tx, cleanup_task) = start_cleanup::<M, C, S>(manager.clone());

        let router = Router::new()
            .route("/healthz", get(|| async {}))
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
            .layer(Extension(ipfs_client))
            .layer(Extension(syndication_tx))
            .layer(Extension(name_system_tx))
            .layer(Extension(cleanup_tx))
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

        Ok(Self {
            router,
            worker_handles: vec![syndication_task, name_system_task, cleanup_task],
        })
    }

    /// Start the gateway server with `listener`, consuming the [Gateway]
    /// object until the process terminates or has an unrecoverable error.
    pub async fn start(self, listener: TcpListener) -> Result<()> {
        // Listener must be set to nonblocking
        // https://docs.rs/tokio/latest/tokio/net/struct.TcpListener.html#method.from_std
        listener.set_nonblocking(true)?;
        let tokio_listener = tokio::net::TcpListener::from_std(listener)?;
        serve(tokio_listener, self.router.into_make_service()).await?;
        for handle in self.worker_handles {
            handle.abort();
        }
        Ok(())
    }
}
