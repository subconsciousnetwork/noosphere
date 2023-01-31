use crate::server::{handlers, routes::Route};
use crate::NameSystem;
use anyhow::Result;
use axum::routing::{delete, get, post};
use axum::{Extension, Router, Server};
use std::net::TcpListener;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct APIServer {
    _handle: tokio::task::JoinHandle<Result<()>>,
}

impl APIServer {
    pub fn serve(ns: Arc<Mutex<NameSystem>>, listener: TcpListener) -> Self {
        let handle = tokio::spawn(async move { run(ns, listener).await });
        APIServer { _handle: handle }
    }
}

async fn run(ns: Arc<Mutex<NameSystem>>, listener: TcpListener) -> Result<()> {
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
        .layer(Extension(ns));

    Server::from_tcp(listener)?
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
