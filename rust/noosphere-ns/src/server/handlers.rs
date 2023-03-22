use crate::{Multiaddr, NameSystem, NameSystemClient, NetworkInfo, NsRecord, Peer, PeerId};
use anyhow::Result;
use axum::response::{IntoResponse, Response};
use axum::{extract::Path, http::StatusCode, Extension, Json};
use noosphere_core::data::Did;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct JsonErr(StatusCode, String);
impl IntoResponse for JsonErr {
    fn into_response(self) -> Response {
        let mut res = Json(Err::<String, String>(self.1)).into_response();
        *res.status_mut() = self.0;
        res
    }
}

type JsonResponse<T> = Result<Json<T>, JsonErr>;

pub async fn get_network_info(
    Extension(name_system): Extension<Arc<Mutex<NameSystem>>>,
) -> JsonResponse<NetworkInfo> {
    let ns = name_system.lock().await;
    let network_info = ns
        .network_info()
        .await
        .map_err(move |error| JsonErr(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(network_info))
}

pub async fn get_peer_id(
    Extension(name_system): Extension<Arc<Mutex<NameSystem>>>,
) -> JsonResponse<PeerId> {
    let ns = name_system.lock().await;
    Ok(Json(ns.peer_id().to_owned()))
}

pub async fn get_peers(
    Extension(name_system): Extension<Arc<Mutex<NameSystem>>>,
) -> JsonResponse<Vec<Peer>> {
    let ns = name_system.lock().await;
    let peers = ns
        .peers()
        .await
        .map_err(move |error| JsonErr(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(peers))
}

pub async fn post_peers(
    Extension(name_system): Extension<Arc<Mutex<NameSystem>>>,
    Path(addr): Path<String>,
) -> JsonResponse<()> {
    let ns = name_system.lock().await;
    let peer_addr = parse_multiaddr(&addr)?;
    ns.add_peers(vec![peer_addr])
        .await
        .map_err(move |error| JsonErr(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(()))
}

pub async fn post_listener(
    Extension(name_system): Extension<Arc<Mutex<NameSystem>>>,
    Path(addr): Path<String>,
) -> JsonResponse<Multiaddr> {
    let ns = name_system.lock().await;
    let listener = parse_multiaddr(&addr)?;
    let address = ns
        .listen(listener)
        .await
        .map_err(move |error| JsonErr(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(address))
}

pub async fn delete_listener(
    Extension(name_system): Extension<Arc<Mutex<NameSystem>>>,
) -> JsonResponse<()> {
    let ns = name_system.lock().await;
    ns.stop_listening()
        .await
        .map_err(move |error| JsonErr(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(()))
}

pub async fn get_address(
    Extension(name_system): Extension<Arc<Mutex<NameSystem>>>,
) -> JsonResponse<Option<Multiaddr>> {
    let ns = name_system.lock().await;
    let address = ns
        .address()
        .await
        .map_err(move |error| JsonErr(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(address))
}

pub async fn get_record(
    Extension(name_system): Extension<Arc<Mutex<NameSystem>>>,
    Path(did): Path<Did>,
) -> JsonResponse<Option<NsRecord>> {
    let ns = name_system.lock().await;
    let record = ns
        .get_record(&did)
        .await
        .map_err(move |error| JsonErr(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(record))
}

pub async fn post_record(
    Extension(name_system): Extension<Arc<Mutex<NameSystem>>>,
    Json(record): Json<NsRecord>,
) -> JsonResponse<()> {
    let ns = name_system.lock().await;
    ns.put_record(record).await.map_err(move |error| {
        error!("500: {}", error);
        JsonErr(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
    })?;
    Ok(Json(()))
}

pub async fn bootstrap(
    Extension(name_system): Extension<Arc<Mutex<NameSystem>>>,
) -> JsonResponse<()> {
    let ns = name_system.lock().await;
    ns.bootstrap()
        .await
        .map_err(move |error| JsonErr(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(()))
}

fn parse_multiaddr(s: &str) -> Result<Multiaddr, JsonErr> {
    // The axum Path parser includes an extra "/" prefix.
    let slice = &s[1..];
    slice
        .parse::<Multiaddr>()
        .map_err(|error| JsonErr(StatusCode::BAD_REQUEST, error.to_string()))
}
