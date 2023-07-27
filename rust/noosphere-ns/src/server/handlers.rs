use crate::{DhtClient, Multiaddr, NameSystem, NetworkInfo, Peer, PeerId};
use anyhow::Result;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use noosphere_core::data::{Did, LinkRecord};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Clone)]
pub struct RouterState {
    pub ns: Arc<NameSystem>,
    pub peer_id: PeerId,
}

pub struct JsonErr(StatusCode, String);
impl IntoResponse for JsonErr {
    fn into_response(self) -> Response {
        let mut res = Json(Err::<String, String>(self.1)).into_response();
        *res.status_mut() = self.0;
        res
    }
}

type JsonResponse<T> = Result<Json<T>, JsonErr>;

pub async fn get_network_info(State(state): State<RouterState>) -> JsonResponse<NetworkInfo> {
    let network_info = state
        .ns
        .network_info()
        .await
        .map_err(move |error| JsonErr(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(network_info))
}

pub async fn get_peer_id(State(state): State<RouterState>) -> JsonResponse<PeerId> {
    Ok(Json(state.peer_id))
}

pub async fn get_peers(State(state): State<RouterState>) -> JsonResponse<Vec<Peer>> {
    let peers = state
        .ns
        .peers()
        .await
        .map_err(move |error| JsonErr(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(peers))
}

pub async fn post_peers(
    State(state): State<RouterState>,
    Path(addr): Path<String>,
) -> JsonResponse<()> {
    let peer_addr = parse_multiaddr(&addr)?;
    state
        .ns
        .add_peers(vec![peer_addr])
        .await
        .map_err(move |error| JsonErr(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(()))
}

pub async fn post_listener(
    State(state): State<RouterState>,
    Path(addr): Path<String>,
) -> JsonResponse<Multiaddr> {
    let listener = parse_multiaddr(&addr)?;
    let address = state
        .ns
        .listen(listener)
        .await
        .map_err(move |error| JsonErr(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(address))
}

pub async fn delete_listener(State(state): State<RouterState>) -> JsonResponse<()> {
    state
        .ns
        .stop_listening()
        .await
        .map_err(move |error| JsonErr(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(()))
}

pub async fn get_address(State(state): State<RouterState>) -> JsonResponse<Option<Multiaddr>> {
    let address = state
        .ns
        .address()
        .await
        .map_err(move |error| JsonErr(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(address))
}

pub async fn get_record(
    State(state): State<RouterState>,
    Path(did): Path<Did>,
) -> JsonResponse<Option<LinkRecord>> {
    let record = state
        .ns
        .get_record(&did)
        .await
        .map_err(move |error| JsonErr(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(record))
}

#[derive(Deserialize, Default)]
pub struct PostRecordQuery {
    quorum: usize,
}

pub async fn post_record(
    State(state): State<RouterState>,
    query: Option<Query<PostRecordQuery>>,
    Json(record): Json<LinkRecord>,
) -> JsonResponse<()> {
    let Query(query) = query.unwrap_or_default();
    state
        .ns
        .put_record(record, query.quorum)
        .await
        .map_err(move |error| JsonErr(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(()))
}

pub async fn bootstrap(State(state): State<RouterState>) -> JsonResponse<()> {
    state
        .ns
        .bootstrap()
        .await
        .map_err(move |error| JsonErr(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(()))
}

fn parse_multiaddr(s: &str) -> Result<Multiaddr, JsonErr> {
    s.parse::<Multiaddr>()
        .map_err(|error| JsonErr(StatusCode::BAD_REQUEST, error.to_string()))
}
