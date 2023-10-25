use anyhow::Result;
use async_trait::async_trait;
use axum::http::{request::Parts, StatusCode};
use noosphere_core::{context::HasMutableSphereContext, data::Did};
use noosphere_storage::Storage;
use std::pin::Pin;
use tokio_stream::Stream;

#[cfg(doc)]
use noosphere_core::context::SphereContext;

/// [Stream] of [SphereContext] from a [GatewayManager].
pub type GatewayManagerSphereStream<'a, C> = dyn Stream<Item = Result<C>> + Send + 'a;

/// [GatewayManager] implementations are used to provide access to managed
/// hosted sphere data and customizations in a Noosphere gateway.
#[async_trait]
pub trait GatewayManager<C, S>: Clone + Send + Sync
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    /// Retrieve a sphere context whose counterpart matches `counterpart`.
    async fn get_sphere_context(&self, counterpart: &Did) -> Result<C>;

    /// Retrieve the identity of the managed sphere's device key from provided `counterpart`,
    /// as a lightweight alternative to fetching the entire sphere context
    /// in [GatewayManager::get_sphere_context].
    async fn get_gateway_identity(&self, counterpart: &Did) -> Result<Did>;

    /// /!\ Iterate over all managed spheres.
    /// /!\ This method is only for the embedded worker implementations in the gateway,
    /// /!\ and not to be used in routes, and will be superceded via #720.
    /// TODO(#720)
    fn experimental_worker_only_iter(&self) -> Pin<Box<GatewayManagerSphereStream<'_, C>>>;

    /// Extract the specified counterpart identity from an [axum] request.
    /// This function should be deterministic in order to take advantage
    /// of caching.
    async fn extract_counterpart(&self, parts: &mut Parts) -> Result<Did, StatusCode>;
}
