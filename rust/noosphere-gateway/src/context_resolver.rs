use anyhow::Result;
use async_trait::async_trait;
use noosphere_core::{context::HasMutableSphereContext, data::Did};
use noosphere_storage::Storage;

#[cfg(doc)]
use noosphere_core::context::SphereContext;

/// Returns a [SphereContext] given a client counterpart [Did],
/// returning the associated managed gateway sphere.
#[async_trait]
pub trait ContextResolver<C, S>: Clone + Send + Sync
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    /// Get a managed [SphereContext] that is associated with
    /// `counterpart` [Did].
    async fn get_context(&self, counterpart: &Did) -> Result<C>;
}
