use crate::{extractors::GatewayScope, ContextResolver};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use noosphere_core::context::HasMutableSphereContext;
use noosphere_core::data::Did;
use noosphere_storage::Storage;

#[cfg(doc)]
use crate::single_tenant::SingleTenantGatewayManager;

/// [ContextResolver] implementation for [SingleTenantGatewayManager].
#[derive(Clone)]
pub struct SingleTenantContextResolver<C, S>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    context: C,
    gateway_scope: GatewayScope<C, S>,
}

impl<C, S> SingleTenantContextResolver<C, S>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    /// Creates a new [SingleTenantContextResolver].
    pub fn new(context: C, gateway_scope: GatewayScope<C, S>) -> Self {
        Self {
            context,
            gateway_scope,
        }
    }
}

#[async_trait]
impl<C, S> ContextResolver<C, S> for SingleTenantContextResolver<C, S>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    async fn get_context(&self, did: &Did) -> Result<C> {
        match &self.gateway_scope.counterpart == did {
            true => Ok(self.context.clone()),
            false => Err(anyhow!(
                "No sphere context found with gateway identity: {did}."
            )),
        }
    }
}
