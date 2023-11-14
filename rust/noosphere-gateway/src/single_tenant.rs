use crate::{GatewayManager, GatewayManagerSphereStream};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use axum::http::{request::Parts, StatusCode};
use noosphere_core::context::HasMutableSphereContext;
use noosphere_core::data::Did;
use noosphere_storage::Storage;
use std::pin::Pin;
use tokio_stream::once;

/// Implements [GatewayManager] for a single sphere context, used in the single-tenant
/// gateway workflow in `orb`.
///
/// Scoping a request to a specific sphere can be handled in different ways, such as
/// subdomain or HTTP header. As a single-tenant gateway only hosts a single sphere,
/// and configuring the server may be unnecessary overhead for an independent operator,
/// the [SingleTenantGatewayManager] counterpart extraction always returns the `counterpart`
/// provided to the constructor.
#[derive(Clone)]
pub struct SingleTenantGatewayManager<C, S>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    context: C,
    identity: Did,
    counterpart: Did,
    marker: std::marker::PhantomData<S>,
}

impl<C, S> SingleTenantGatewayManager<C, S>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    /// Create a new [SingleTenantGatewayManager], implementing [GatewayManager] for a single sphere `context`.
    pub async fn new(context: C, counterpart: Did) -> Result<Self> {
        let identity = context.sphere_context().await?.author().did().await?;
        Ok(SingleTenantGatewayManager {
            identity,
            context,
            counterpart,
            marker: std::marker::PhantomData,
        })
    }
}

#[async_trait]
impl<C, S> GatewayManager<C, S> for SingleTenantGatewayManager<C, S>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    async fn get_sphere_context(&self, counterpart: &Did) -> Result<C> {
        match &self.counterpart == counterpart {
            true => Ok(self.context.clone()),
            false => Err(anyhow!(
                "No sphere context found for counterpart: {counterpart}."
            )),
        }
    }

    async fn get_gateway_identity(&self, counterpart: &Did) -> Result<Did> {
        match &self.counterpart == counterpart {
            true => Ok(self.identity.clone()),
            false => Err(anyhow!(
                "No sphere identity found for counterpart: {counterpart}."
            )),
        }
    }

    fn experimental_worker_only_iter(&self) -> Pin<Box<GatewayManagerSphereStream<'_, C>>> {
        Box::pin(once(Ok(self.context.clone())))
    }

    async fn extract_counterpart(&self, _: &mut Parts) -> Result<Did, StatusCode> {
        Ok(self.counterpart.clone())
    }
}
