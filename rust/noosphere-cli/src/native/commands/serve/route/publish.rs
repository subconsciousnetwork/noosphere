use anyhow::Result;
use axum::{extract::ContentLengthLimit, http::StatusCode, Extension};

use noosphere_api::data::{PublishBody, PublishResponse};
use noosphere_core::authority::{SphereAction, SphereReference};
use noosphere_ns::{DHTKeyMaterial, NSRecord};
use tokio::sync::mpsc::UnboundedSender;
use ucan::capability::{Capability, Resource, With};

use crate::native::commands::serve::{
    authority::GatewayAuthority, extractor::Cbor, gateway::GatewayScope, name_system::NSJob,
};

//use axum::debug_handler;
//#[debug_handler]
pub async fn publish_route<K>(
    authority: GatewayAuthority<K>,
    ContentLengthLimit(Cbor(publish_body)): ContentLengthLimit<Cbor<PublishBody>, { 1024 * 5000 }>,
    Extension(scope): Extension<GatewayScope>,
    Extension(ns_tx): Extension<Option<UnboundedSender<NSJob>>>,
) -> Result<Cbor<PublishResponse>, StatusCode>
where
    K: DHTKeyMaterial + 'static,
{
    debug!("Invoking publish route...");

    if ns_tx.is_none() {
        return Err(StatusCode::BAD_REQUEST);
    }

    if &publish_body.sphere != &scope.counterpart {
        return Err(StatusCode::FORBIDDEN);
    }

    authority.try_authorize(&Capability {
        with: With::Resource {
            kind: Resource::Scoped(SphereReference {
                did: scope.counterpart.to_string(),
            }),
        },
        can: SphereAction::Publish,
    })?;

    let publish_token = NSRecord::try_from(publish_body.publish_token).map_err(|error| {
        error!("{:?}", error);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if let Err(e) = ns_tx.unwrap().send(NSJob::PutRecord { publish_token }) {
        warn!("Failed to queue name system job: {}", e);
    }

    Ok(Cbor(PublishResponse {}))
}
