use std::str::FromStr;

use axum::{extract::Query, http::StatusCode, response::IntoResponse, Extension};
use cid::Cid;
use noosphere::{
    authority::{SphereAction, SphereReference},
    view::Sphere,
};
use noosphere_api::data::{FetchParameters, FetchResponse};
use noosphere_storage::{db::SphereDb, native::NativeStore};
use ucan::capability::{Capability, Resource, With};

use crate::native::commands::serve::{
    authority::GatewayAuthority, extractor::Cbor, gateway::GatewayScope,
};

pub async fn fetch_route(
    authority: GatewayAuthority,
    Query(FetchParameters { since }): Query<FetchParameters>,
    Extension(scope): Extension<GatewayScope>,
    Extension(db): Extension<SphereDb<NativeStore>>,
) -> Result<impl IntoResponse, StatusCode> {
    authority.try_authorize(&Capability {
        with: With::Resource {
            kind: Resource::Scoped(SphereReference {
                did: scope.counterpart.to_string(),
            }),
        },
        can: SphereAction::Fetch,
    })?;

    let ancestor_cid = match since {
        Some(since) => Some(Cid::from_str(&since).map_err(|_| StatusCode::BAD_REQUEST)?),
        None => None,
    };

    debug!("Resolving local counterpart sphere version...");

    let sphere_cid = db
        .require_version(&scope.counterpart)
        .await
        .map_err(|error| {
            error!("{:?}", error);
            StatusCode::NOT_FOUND
        })?;

    let sphere = Sphere::at(&sphere_cid, &db);

    debug!("Bundling revisions since {:?}...", ancestor_cid);
    let bundle = sphere
        .try_bundle_until_ancestor(ancestor_cid.as_ref())
        .await
        .map_err(|error| {
            error!("{:?}", error);
            StatusCode::NOT_FOUND
        })?;

    // TODO: Paged fetching...

    Ok(Cbor(FetchResponse {
        tip: sphere_cid,
        blocks: bundle,
    }))
}
