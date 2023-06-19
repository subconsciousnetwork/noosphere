use crate::{DhtClient, Multiaddr, NameSystem, NetworkInfo, Peer, PeerId};
use anyhow::Result;
use axum::response::{IntoResponse, Response};
use axum::{
    extract::{Path, Query},
    http::StatusCode,
    Extension, Json,
};
use noosphere_core::data::{Did, LinkRecord};
use serde::Deserialize;
use std::sync::Arc;

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
    Extension(name_system): Extension<Arc<NameSystem>>,
) -> JsonResponse<NetworkInfo> {
    let network_info = name_system
        .network_info()
        .await
        .map_err(move |error| JsonErr(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(network_info))
}

pub async fn get_peer_id(Extension(peer_id): Extension<PeerId>) -> JsonResponse<PeerId> {
    Ok(Json(peer_id))
}

pub async fn get_peers(
    Extension(name_system): Extension<Arc<NameSystem>>,
) -> JsonResponse<Vec<Peer>> {
    let peers = name_system
        .peers()
        .await
        .map_err(move |error| JsonErr(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(peers))
}

pub async fn post_peers(
    Extension(name_system): Extension<Arc<NameSystem>>,
    Path(addr): Path<String>,
) -> JsonResponse<()> {
    let peer_addr = parse_multiaddr(&addr)?;
    name_system
        .add_peers(vec![peer_addr])
        .await
        .map_err(move |error| JsonErr(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(()))
}

pub async fn post_listener(
    Extension(name_system): Extension<Arc<NameSystem>>,
    Path(addr): Path<String>,
) -> JsonResponse<Multiaddr> {
    let listener = parse_multiaddr(&addr)?;
    let address = name_system
        .listen(listener)
        .await
        .map_err(move |error| JsonErr(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(address))
}

pub async fn delete_listener(
    Extension(name_system): Extension<Arc<NameSystem>>,
) -> JsonResponse<()> {
    name_system
        .stop_listening()
        .await
        .map_err(move |error| JsonErr(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(()))
}

pub async fn get_address(
    Extension(name_system): Extension<Arc<NameSystem>>,
) -> JsonResponse<Option<Multiaddr>> {
    let address = name_system
        .address()
        .await
        .map_err(move |error| JsonErr(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(address))
}

pub async fn get_record(
    Extension(name_system): Extension<Arc<NameSystem>>,
    Path(did): Path<Did>,
) -> JsonResponse<Option<LinkRecord>> {
    let record = name_system
        .get_record(&did)
        .await
        .map_err(move |error| JsonErr(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(record))
}

#[derive(Deserialize)]
pub struct PostRecordQuery {
    quorum: usize,
}

pub async fn post_record(
    Extension(name_system): Extension<Arc<NameSystem>>,
    Json(record): Json<LinkRecord>,
    Query(query): Query<PostRecordQuery>,
) -> JsonResponse<()> {
    name_system
        .put_record(record, query.quorum)
        .await
        .map_err(move |error| {
            warn!("Error: {}", error);
            JsonErr(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
        })?;
    Ok(Json(()))
}

pub async fn bootstrap(Extension(name_system): Extension<Arc<NameSystem>>) -> JsonResponse<()> {
    name_system
        .bootstrap()
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
