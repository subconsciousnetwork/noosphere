use crate::{Multiaddr, NSRecord, NameSystem, NameSystemClient, NetworkInfo, Peer, PeerId};
use anyhow::{Error, Result};
use axum::{extract::Path, http::StatusCode, Extension, Json};
use noosphere_core::data::Did;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::*;

pub async fn get_network_info(
    Extension(name_system): Extension<Arc<Mutex<NameSystem>>>,
) -> Result<Json<NetworkInfo>, StatusCode> {
    let ns = name_system.lock().await;
    let network_info = api_call(ns.network_info(), 500).await?;
    Ok(Json(network_info))
}

pub async fn get_peer_id(
    Extension(name_system): Extension<Arc<Mutex<NameSystem>>>,
) -> Result<Json<PeerId>, StatusCode> {
    let ns = name_system.lock().await;
    Ok(Json(ns.peer_id().to_owned()))
}

pub async fn get_peers(
    Extension(name_system): Extension<Arc<Mutex<NameSystem>>>,
) -> Result<Json<Vec<Peer>>, StatusCode> {
    let ns = name_system.lock().await;
    let peers = api_call(ns.peers(), 500).await?;
    Ok(Json(peers))
}

pub async fn post_peers(
    Extension(name_system): Extension<Arc<Mutex<NameSystem>>>,
    Path(addr): Path<String>,
) -> Result<Json<()>, StatusCode> {
    let ns = name_system.lock().await;
    let peer_addr = parse_multiaddr(&addr)?;
    api_call(ns.add_peers(vec![peer_addr]), 500).await?;
    Ok(Json(()))
}

pub async fn post_listener(
    Extension(name_system): Extension<Arc<Mutex<NameSystem>>>,
    Path(addr): Path<String>,
) -> Result<Json<Multiaddr>, StatusCode> {
    let ns = name_system.lock().await;
    let listener = parse_multiaddr(&addr)?;
    let address = api_call(ns.listen(listener), 500).await?;
    Ok(Json(address))
}

pub async fn delete_listener(
    Extension(name_system): Extension<Arc<Mutex<NameSystem>>>,
) -> Result<Json<()>, StatusCode> {
    let ns = name_system.lock().await;
    api_call(ns.stop_listening(), 500).await?;
    Ok(Json(()))
}

pub async fn get_address(
    Extension(name_system): Extension<Arc<Mutex<NameSystem>>>,
) -> Result<Json<Option<Multiaddr>>, StatusCode> {
    let ns = name_system.lock().await;
    let address = api_call(ns.address(), 500).await?;
    Ok(Json(address))
}

pub async fn get_record(
    Extension(name_system): Extension<Arc<Mutex<NameSystem>>>,
    Path(did): Path<Did>,
) -> Result<Json<Option<NSRecord>>, StatusCode> {
    let ns = name_system.lock().await;
    let record = api_call(ns.get_record(&did), 500).await?;
    Ok(Json(record))
}

pub async fn post_record(
    Extension(name_system): Extension<Arc<Mutex<NameSystem>>>,
    Json(record): Json<NSRecord>,
) -> Result<Json<()>, StatusCode> {
    let ns = name_system.lock().await;
    api_call(ns.put_record(record), 500).await?;
    Ok(Json(()))
}

pub async fn bootstrap(
    Extension(name_system): Extension<Arc<Mutex<NameSystem>>>,
) -> Result<Json<()>, StatusCode> {
    let ns = name_system.lock().await;
    api_call(ns.bootstrap(), 500).await?;
    Ok(Json(()))
}

async fn api_call<T>(
    promise: impl Future<Output = Result<T, Error>>,
    code: u16,
) -> Result<T, StatusCode> {
    promise.await.map_err(move |error| {
        error!("{:?}", error);
        StatusCode::from_u16(code).unwrap()
    })
}

fn parse_multiaddr(s: &str) -> Result<Multiaddr, StatusCode> {
    // The axum Path parser includes an extra "/" prefix.
    let slice = &s[1..];
    slice.parse().map_err(|error| {
        error!("{:?}", error);
        StatusCode::BAD_REQUEST
    })
}
