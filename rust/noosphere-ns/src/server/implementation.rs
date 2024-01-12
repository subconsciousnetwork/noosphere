use crate::server::{handlers, routes::Route};
use crate::{DhtClient, NameSystem};
use anyhow::Result;
use axum::routing::{delete, get, post};
use axum::{Router, Server};
use axum_tracing_opentelemetry::middleware::{OtelAxumLayer, OtelInResponseLayer};
use std::net::TcpListener;
use std::sync::Arc;
use tower_http::trace::TraceLayer;

pub async fn start_name_system_api_server(
    ns: Arc<NameSystem>,
    listener: TcpListener,
) -> Result<()> {
    let peer_id = ns.peer_id().to_owned();

    let app = Router::new()
        .route(
            &Route::NetworkInfo.to_string(),
            get(handlers::get_network_info),
        )
        .route(&Route::GetPeerId.to_string(), get(handlers::get_peer_id))
        .route(&Route::GetPeers.to_string(), get(handlers::get_peers))
        .route(&Route::AddPeers.to_string(), post(handlers::post_peers))
        .route(&Route::Listen.to_string(), post(handlers::post_listener))
        .route(
            &Route::StopListening.to_string(),
            delete(handlers::delete_listener),
        )
        .route(&Route::Address.to_string(), get(handlers::get_address))
        .route(&Route::GetRecord.to_string(), get(handlers::get_record))
        .route(&Route::PostRecord.to_string(), post(handlers::post_record))
        .route(&Route::Bootstrap.to_string(), post(handlers::bootstrap))
        .layer(OtelInResponseLayer) // include trace context in response
        .layer(OtelAxumLayer::default()) // initialize otel trace on incoming request
        .layer(TraceLayer::new_for_http())
        .with_state(handlers::RouterState { ns, peer_id });

    Server::from_tcp(listener)?
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

pub struct ApiServer {
    #[allow(dead_code)]
    handle: tokio::task::JoinHandle<Result<()>>,
}

impl ApiServer {
    pub fn serve(ns: Arc<NameSystem>, listener: TcpListener) -> Self {
        let handle = tokio::spawn(async move {
            start_name_system_api_server(ns, listener).await?;
            Ok(())
        });
        ApiServer { handle }
    }
}
