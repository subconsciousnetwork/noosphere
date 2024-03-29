use std::pin::Pin;

use anyhow::Result;

use axum::{body::Body, extract::Query, http::StatusCode, Extension};
use bytes::Bytes;
use noosphere_core::{
    api::v0alpha1::FetchParameters,
    authority::SphereAbility,
    context::HasMutableSphereContext,
    data::{Did, Link, MemoIpld},
    stream::{memo_history_stream, to_car_stream},
    view::Sphere,
};
use noosphere_ipfs::{IpfsStore, KuboClient};
use noosphere_storage::{BlockStoreRetry, SphereDb, Storage};
use tokio_stream::{Stream, StreamExt};

use crate::{
    extractors::{GatewayAuthority, GatewayScope},
    GatewayManager,
};

#[instrument(level = "debug", skip(gateway_scope, authority, ipfs_client))]
pub async fn fetch_route<M, C, S>(
    gateway_scope: GatewayScope<C, S>,
    authority: GatewayAuthority<M, C, S>,
    Query(FetchParameters { since }): Query<FetchParameters>,
    Extension(ipfs_client): Extension<KuboClient>,
) -> Result<Body, StatusCode>
where
    M: GatewayManager<C, S> + 'static,
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    let gateway_sphere = authority
        .try_authorize(&gateway_scope, SphereAbility::Fetch)
        .await?;

    let sphere_context = gateway_sphere.sphere_context().await.map_err(|err| {
        error!("{err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let db = sphere_context.db();
    let identity = sphere_context.identity();
    let stream = generate_fetch_stream(
        &gateway_scope.counterpart,
        identity,
        since.as_ref(),
        db,
        ipfs_client,
    )
    .await
    .map_err(|err| {
        error!("{err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Body::from_stream(stream))
}

/// Generates a CAR stream that can be used as a the streaming body of a
/// gateway fetch route response
pub async fn generate_fetch_stream<S>(
    counterpart: &Did,
    identity: &Did,
    since: Option<&Link<MemoIpld>>,
    db: &SphereDb<S>,
    ipfs_client: KuboClient,
) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>>
where
    S: Storage + 'static,
{
    let latest_local_sphere_cid = db.require_version(identity).await?.into();

    debug!("The latest gateway sphere version is {latest_local_sphere_cid}...");

    if Some(&latest_local_sphere_cid) == since {
        debug!(
            "No changes since {}",
            since
                .map(|cid| cid.to_string())
                .unwrap_or_else(|| "the beginning...".into())
        );
        return Ok(Box::pin(to_car_stream(vec![], tokio_stream::empty())));
    }

    debug!(
        "Streaming gateway sphere revisions from {} to {}...",
        latest_local_sphere_cid,
        since
            .map(|cid| cid.to_string())
            .unwrap_or_else(|| "the beginning".into())
    );

    let store = BlockStoreRetry::from(IpfsStore::new(db.clone(), Some(ipfs_client)));

    let stream = memo_history_stream(store.clone(), &latest_local_sphere_cid, since, false);

    debug!("Resolving latest counterpart sphere version...");

    let latest_local_sphere = Sphere::at(&latest_local_sphere_cid, db);
    match latest_local_sphere
        .get_content()
        .await?
        .get(counterpart)
        .await?
    {
        Some(latest_counterpart_sphere_cid) => {
            debug!("Resolving oldest counterpart sphere version...");

            let since = match since {
                Some(since_local_sphere_cid) => {
                    let since_local_sphere = Sphere::at(since_local_sphere_cid, db);
                    let links = since_local_sphere.get_content().await?;
                    links.get(counterpart).await?.cloned()
                }
                None => None,
            };

            debug!(
                "Streaming counterpart revisions from {} to {}...",
                latest_counterpart_sphere_cid,
                since
                    .as_ref()
                    .map(|cid| cid.to_string())
                    .unwrap_or_else(|| "the beginning".into())
            );

            return Ok(Box::pin(to_car_stream(
                vec![latest_local_sphere_cid.into()],
                stream.merge(memo_history_stream(
                    store,
                    latest_counterpart_sphere_cid,
                    since.as_ref(),
                    false,
                )),
            )));
        }
        None => {
            warn!("No revisions found for counterpart {}!", counterpart);
            Ok(Box::pin(to_car_stream(
                vec![latest_local_sphere_cid.into()],
                stream,
            )))
        }
    }
}
