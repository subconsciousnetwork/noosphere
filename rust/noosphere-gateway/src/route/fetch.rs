use anyhow::Result;

use axum::{extract::Query, http::StatusCode, response::IntoResponse, Extension};
use cid::Cid;
use libipld_cbor::DagCborCodec;
use noosphere_api::data::{FetchParameters, FetchResponse};
use noosphere_core::{
    authority::{SphereAction, SphereReference},
    data::Bundle,
    view::Sphere,
};
use noosphere_sphere::HasSphereContext;
use noosphere_storage::{block_serialize, SphereDb, Storage};
use ucan::{
    capability::{Capability, Resource, With},
    crypto::KeyMaterial,
};

use crate::{authority::GatewayAuthority, extractor::Cbor, GatewayScope};

pub async fn fetch_route<H, K, S>(
    authority: GatewayAuthority<K>,
    Query(FetchParameters { since }): Query<FetchParameters>,
    Extension(scope): Extension<GatewayScope>,
    Extension(sphere_context): Extension<H>,
) -> Result<impl IntoResponse, StatusCode>
where
    H: HasSphereContext<K, S>,
    K: KeyMaterial + Clone,
    S: Storage,
{
    authority.try_authorize(&Capability {
        with: With::Resource {
            kind: Resource::Scoped(SphereReference {
                did: scope.counterpart.to_string(),
            }),
        },
        can: SphereAction::Fetch,
    })?;
    let sphere_context = sphere_context
        .sphere_context()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let db = sphere_context.db();

    let response = match generate_fetch_bundle(&scope, since.as_ref(), db)
        .await
        .map_err(|error| {
            error!("{:?}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })? {
        Some((tip, bundle)) => FetchResponse::NewChanges {
            tip,
            blocks: bundle,
        },
        None => FetchResponse::UpToDate,
    };

    Ok(Cbor(response))
}

pub async fn generate_fetch_bundle<S>(
    scope: &GatewayScope,
    since: Option<&Cid>,
    db: &SphereDb<S>,
) -> Result<Option<(Cid, Bundle)>>
where
    S: Storage,
{
    debug!("Resolving latest local sphere version...");

    let latest_local_sphere_cid = db.require_version(&scope.identity).await?;

    if Some(&latest_local_sphere_cid) == since {
        debug!(
            "No changes since {}",
            since
                .map(|cid| cid.to_string())
                .unwrap_or("the beginning...".into())
        );
        return Ok(None);
    }

    let latest_local_sphere = Sphere::at(&latest_local_sphere_cid, db);

    debug!(
        "Bundling local sphere revisions since {:?}...",
        since
            .map(|cid| cid.to_string())
            .unwrap_or("the beginning".into())
    );

    let mut bundle = latest_local_sphere.bundle_until_ancestor(since).await?;

    debug!("Resolving latest counterpart sphere version...");

    match latest_local_sphere
        .get_links()
        .await?
        .get(&scope.counterpart)
        .await?
        .cloned()
    {
        Some(latest_counterpart_sphere_cid) => {
            debug!("Resolving oldest counterpart sphere version...");

            let since = match since {
                Some(since_local_sphere_cid) => {
                    let since_local_sphere = Sphere::at(since_local_sphere_cid, db);
                    let links = since_local_sphere.get_links().await?;
                    links.get_as_cid::<DagCborCodec>(&scope.counterpart).await?
                }
                None => None,
            };

            debug!(
                "Bundling counterpart revisions from {} to {}...",
                latest_counterpart_sphere_cid,
                since
                    .map(|cid| cid.to_string())
                    .unwrap_or("the beginning".into())
            );

            bundle.merge(
                Sphere::at(&latest_counterpart_sphere_cid, db)
                    .bundle_until_ancestor(since.as_ref())
                    .await?,
            )
        }
        None => {
            warn!("No revisions found for counterpart {}!", scope.counterpart);
        }
    };

    Ok(Some((latest_local_sphere_cid, bundle)))
}
