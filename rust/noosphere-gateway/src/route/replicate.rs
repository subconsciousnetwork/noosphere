use std::time::Duration;

use anyhow::Result;
use axum::{body::StreamBody, extract::Path, http::StatusCode, Extension};
use bytes::Bytes;
use cid::Cid;
use noosphere_core::authority::{SphereAction, SphereReference};
use noosphere_ipfs::{IpfsStore, KuboClient};
use noosphere_sphere::{car_stream, HasMutableSphereContext};
use noosphere_storage::{BlockStoreRetry, Storage};
use tokio_stream::Stream;
use ucan::{
    capability::{Capability, Resource, With},
    crypto::KeyMaterial,
};

use crate::{authority::GatewayAuthority, GatewayScope};

/// Invoke to get a streamed CARv1 response that represents all the blocks
/// needed to manifest the content associated with the given CID path parameter.
/// The CID should refer to the memo that wraps the content. The content-type
/// header is used to determine how to gather the associated blocks to be
/// streamed by to the requesting client. Invoker must have authorization to
/// fetch from the gateway.
pub async fn replicate_route<C, K, S>(
    authority: GatewayAuthority<K>,
    // NOTE: Cannot go from string to CID via serde
    Path(memo_version): Path<String>,
    Extension(scope): Extension<GatewayScope>,
    Extension(ipfs_client): Extension<KuboClient>,
    Extension(sphere_context): Extension<C>,
) -> Result<StreamBody<impl Stream<Item = Result<Bytes, std::io::Error>>>, StatusCode>
where
    C: HasMutableSphereContext<K, S> + 'static,
    K: KeyMaterial + Clone,
    S: Storage + 'static,
{
    debug!("Invoking replicate route...");
    let memo_version = Cid::try_from(memo_version).map_err(|error| {
        warn!("{}", error);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    authority.try_authorize(&Capability {
        with: With::Resource {
            kind: Resource::Scoped(SphereReference {
                did: scope.counterpart.to_string(),
            }),
        },
        can: SphereAction::Fetch,
    })?;

    let db = sphere_context
        .sphere_context()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .db()
        .clone();
    let store = BlockStoreRetry::new(
        IpfsStore::new(db, Some(ipfs_client)),
        6,
        Duration::from_secs(10),
    );

    Ok(StreamBody::new(car_stream(store, memo_version)))
}
