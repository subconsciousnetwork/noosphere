use std::{pin::Pin, time::Duration};

use anyhow::Result;

use axum::{body::StreamBody, extract::Query, http::StatusCode, Extension};
use bytes::Bytes;
use noosphere_api::data::FetchParameters;
use noosphere_core::{
    authority::{SphereAction, SphereReference},
    data::{Link, MemoIpld},
    view::Sphere,
};
use noosphere_ipfs::{IpfsStore, KuboClient};
use noosphere_sphere::{car_stream, memo_history_stream, HasSphereContext};
use noosphere_storage::{BlockStoreRetry, SphereDb, Storage};
use tokio_stream::{Stream, StreamExt};
use ucan::{
    capability::{Capability, Resource, With},
    crypto::KeyMaterial,
};

use crate::{authority::GatewayAuthority, GatewayScope};

#[instrument(level = "debug", skip(authority, scope, sphere_context, ipfs_client))]
pub async fn fetch_route<C, K, S>(
    authority: GatewayAuthority<K>,
    Query(FetchParameters { since }): Query<FetchParameters>,
    Extension(scope): Extension<GatewayScope>,
    Extension(ipfs_client): Extension<KuboClient>,
    Extension(sphere_context): Extension<C>,
) -> Result<StreamBody<impl Stream<Item = Result<Bytes, std::io::Error>>>, StatusCode>
where
    C: HasSphereContext<K, S>,
    K: KeyMaterial + Clone,
    S: Storage + 'static,
{
    authority.try_authorize(&Capability {
        with: With::Resource {
            kind: Resource::Scoped(SphereReference {
                did: scope.counterpart.to_string(),
            }),
        },
        can: SphereAction::Fetch,
    })?;
    let sphere_context = sphere_context.sphere_context().await.map_err(|err| {
        error!("{err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let db = sphere_context.db();

    let stream = generate_fetch_stream(&scope, since.as_ref(), db, ipfs_client)
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
    scope: &GatewayScope,
    since: Option<&Link<MemoIpld>>,
    db: &SphereDb<S>,
    ipfs_client: KuboClient,
) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>>
where
    S: Storage + 'static,
{
    debug!("Resolving latest local sphere version...");

    let latest_local_sphere_cid = db.require_version(&scope.identity).await?.into();

    if Some(&latest_local_sphere_cid) == since {
        debug!(
            "No changes since {}",
            since
                .map(|cid| cid.to_string())
                .unwrap_or_else(|| "the beginning...".into())
        );
        return Ok(Box::pin(car_stream(vec![], tokio_stream::empty())));
    }

    debug!(
        "Streaming gateway sphere revisions since {:?}...",
        since
            .map(|cid| cid.to_string())
            .unwrap_or_else(|| "the beginning".into())
    );

    let store = BlockStoreRetry::new(
        IpfsStore::new(db.clone(), Some(ipfs_client)),
        6,
        Duration::from_secs(10),
    );

    let stream = memo_history_stream(store.clone(), &latest_local_sphere_cid, since);

    debug!("Resolving latest counterpart sphere version...");

    let latest_local_sphere = Sphere::at(&latest_local_sphere_cid, db);
    match latest_local_sphere
        .get_content()
        .await?
        .get(&scope.counterpart)
        .await?
    {
        Some(latest_counterpart_sphere_cid) => {
            debug!("Resolving oldest counterpart sphere version...");

            let since = match since {
                Some(since_local_sphere_cid) => {
                    let since_local_sphere = Sphere::at(since_local_sphere_cid, db);
                    let links = since_local_sphere.get_content().await?;
                    links.get(&scope.counterpart).await?.cloned()
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

            return Ok(Box::pin(car_stream(
                vec![latest_local_sphere_cid.into()],
                stream.merge(memo_history_stream(
                    store,
                    &latest_counterpart_sphere_cid,
                    since.as_ref(),
                )),
            )));
        }
        None => {
            warn!("No revisions found for counterpart {}!", scope.counterpart);
            return Ok(Box::pin(car_stream(
                vec![latest_local_sphere_cid.into()],
                stream,
            )));
        }
    };
}
