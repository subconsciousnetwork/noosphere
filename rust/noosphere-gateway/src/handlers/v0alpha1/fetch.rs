use std::pin::Pin;

use anyhow::Result;

use axum::{body::StreamBody, extract::Query, http::StatusCode, Extension};
use bytes::Bytes;
use noosphere_core::{
    api::v0alpha1::FetchParameters,
    authority::{generate_capability, SphereAbility},
    context::HasMutableSphereContext,
    data::{Did, Link, MemoIpld},
    stream::{memo_history_stream, to_car_stream},
    view::Sphere,
};
use noosphere_ipfs::{IpfsStore, KuboClient};
use noosphere_storage::{BlockStoreRetry, SphereDb, Storage};
use tokio_stream::{Stream, StreamExt};

use crate::extractors::{GatewayAuthority, GatewayScope, SphereExtractor};

#[instrument(
    level = "debug",
    skip(authority, sphere_extractor, gateway_scope, ipfs_client)
)]
pub async fn fetch_route<C, S>(
    authority: GatewayAuthority,
    sphere_extractor: SphereExtractor<C, S>,
    gateway_scope: GatewayScope<C, S>,
    Query(FetchParameters { since }): Query<FetchParameters>,
    Extension(ipfs_client): Extension<KuboClient>,
) -> Result<StreamBody<impl Stream<Item = Result<Bytes, std::io::Error>>>, StatusCode>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    let mut gateway_sphere = sphere_extractor.into_inner();
    let counterpart = &gateway_scope.counterpart;
    authority
        .try_authorize(
            &mut gateway_sphere,
            counterpart,
            &generate_capability(counterpart.as_str(), SphereAbility::Fetch),
        )
        .await?;

    let sphere_context = gateway_sphere.sphere_context().await.map_err(|err| {
        error!("{err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let db = sphere_context.db();
    let identity = sphere_context.identity();
    let stream = generate_fetch_stream(counterpart, identity, since.as_ref(), db, ipfs_client)
        .await
        .map_err(|err| {
            error!("{err}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(StreamBody::new(stream))
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
