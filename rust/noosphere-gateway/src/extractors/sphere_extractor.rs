use std::{marker::PhantomData, sync::Arc};

use crate::{extractors::map_bad_request, GatewayManager};
use anyhow::Result;
use async_trait::async_trait;
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use noosphere_core::context::HasMutableSphereContext;
use noosphere_storage::Storage;

#[cfg(doc)]
use noosphere_core::context::SphereContext;

/// A wrapper type around a [SphereContext] scoped by the counterpart
/// parsed by [GatewayManager::extract_counterpart].
#[derive(Clone)]
pub struct SphereExtractor<C, S>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    sphere: C,
    storage_marker: PhantomData<S>,
}

impl<C, S> SphereExtractor<C, S>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    /// Returns the inner sphere.
    pub fn into_inner(self) -> C {
        self.sphere
    }
}

#[async_trait]
impl<M, C, S> FromRequestParts<Arc<M>> for SphereExtractor<C, S>
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
        let sphere = state
            .get_sphere_context(&counterpart)
            .await
            .map_err(map_bad_request)?;

        Ok(SphereExtractor {
            sphere,
            storage_marker: PhantomData,
        })
    }
}
