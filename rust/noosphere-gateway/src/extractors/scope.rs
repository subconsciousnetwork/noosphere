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

/// Represents the scope of a gateway request as a counterpart [Did],
/// and the corresponding managed sphere's author/device key,
/// the gateway identity.
///
/// Extracting a [GatewayScope] is efficient, and does not open
/// a [SphereContext].
#[derive(Clone)]
pub struct GatewayScope<C, S> {
    /// [Did] of the client counterpart sphere.
    pub counterpart: Did,
    /// [Did] of the author of the managed gateway sphere.
    pub gateway_identity: Did,
    sphere_context_marker: PhantomData<C>,
    storage_marker: PhantomData<S>,
}

impl<C, S> GatewayScope<C, S> {
    /// Creates a new [GatewayScope].
    pub fn new(gateway_identity: Did, counterpart: Did) -> Self {
        Self {
            gateway_identity,
            counterpart,
            sphere_context_marker: PhantomData,
            storage_marker: PhantomData,
        }
    }
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
        let (gateway_identity, counterpart) = state.gateway_scope(parts).await?;
        Ok(GatewayScope::new(gateway_identity, counterpart))
    }
}
