use crate::jobs::JobClient;
use anyhow::Result;
use async_trait::async_trait;
use axum::http::{request::Parts, StatusCode};
use noosphere_core::{context::HasMutableSphereContext, data::Did};
use noosphere_storage::{Storage, UcanStore};
use url::Url;

#[cfg(doc)]
use noosphere_core::context::SphereContext;

/// [GatewayManager] implementations are used to provide access to managed
/// hosted sphere data and customizations in a Noosphere gateway.
#[async_trait]
pub trait GatewayManager<C, S>: Clone + Send + Sync
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    /// Type of [JobClient] for this [GatewayManager].
    type JobClient: JobClient + Clone;

    /// Returns the [JobClient] for the gateway.
    fn job_client(&self) -> Self::JobClient;

    /// The [Url] of an IPFS Kubo instance.
    fn ipfs_api_url(&self) -> Url;

    /// An optional [Url] to configure CORS layers.
    fn cors_origin(&self) -> Option<Url>;

    /// Retrieve a [UcanStore] for `sphere_identity`.
    async fn ucan_store(&self, sphere_identity: &Did) -> Result<UcanStore<S::BlockStore>>;

    /// Retrieve a sphere context that maps to `sphere_identity`.
    async fn sphere_context(&self, sphere_identity: &Did) -> Result<C>;

    /// Extract the specified gateway identity (0) and counterpart (1)
    /// from an [axum] request. This function should be deterministic in
    /// order to take advantage of caching.
    async fn gateway_scope(&self, parts: &mut Parts) -> Result<(Did, Did, Did), StatusCode>;
}
