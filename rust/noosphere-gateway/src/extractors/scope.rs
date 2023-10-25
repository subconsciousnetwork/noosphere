use std::{marker::PhantomData, sync::Arc};

use crate::GatewayManager;
use anyhow::Result;
use async_trait::async_trait;
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use noosphere_core::{context::HasMutableSphereContext, data::Did};
use noosphere_storage::Storage;

#[cfg(doc)]
use noosphere_core::context::SphereContext;

use super::map_bad_request;

/// Represents the scope of a gateway request as a counterpart [Did],
/// and the corresponding managed sphere's author/device key,
/// the gateway identity.
///
/// Extracting a [GatewayScope] is efficient, and does not open
/// a [SphereContext].
pub struct GatewayScope<C, S> {
    pub counterpart: Did,
    pub gateway_identity: Did,
    sphere_context_marker: PhantomData<C>,
    storage_marker: PhantomData<S>,
}

#[async_trait]
impl<M, C, S> FromRequestParts<Arc<M>> for GatewayScope<C, S>
where
    M: GatewayManager<C, S> + 'static,
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<M>,
    ) -> Result<Self, Self::Rejection> {
        let counterpart = state.extract_counterpart(parts).await?;
        let gateway_identity = state
            .get_gateway_identity(&counterpart)
            .await
            .map_err(map_bad_request)?;

        Ok(GatewayScope {
            counterpart,
            gateway_identity,
            sphere_context_marker: PhantomData,
            storage_marker: PhantomData,
        })
    }
}
