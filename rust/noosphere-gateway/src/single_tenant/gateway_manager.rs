use crate::{
    extractors::GatewayScope, single_tenant::SingleTenantJobClient, ContextResolver,
    GatewayManager, SingleTenantContextResolver,
};
use anyhow::Result;
use async_trait::async_trait;
use axum::http::{request::Parts, StatusCode};
use noosphere_core::context::HasMutableSphereContext;
use noosphere_core::data::Did;
use noosphere_ipfs::KuboClient;
use noosphere_storage::{Storage, UcanStore};
use url::Url;

/// Implements [GatewayManager] for a single sphere context, used in the single-tenant
/// gateway workflow in `orb`.
#[derive(Clone)]
pub struct SingleTenantGatewayManager<C, S>
where
    C: HasMutableSphereContext<S> + 'static,
    S: Storage + 'static,
{
    context: C,
    gateway_scope: GatewayScope<C, S>,
    context_resolver: SingleTenantContextResolver<C, S>,
    job_client: SingleTenantJobClient<C, S>,
    ipfs_api: Url,
    cors_origin: Option<Url>,
    marker: std::marker::PhantomData<S>,
}

impl<C, S> SingleTenantGatewayManager<C, S>
where
    C: HasMutableSphereContext<S> + 'static,
    S: Storage + 'static,
{
    /// Create a new [SingleTenantGatewayManager], implementing [GatewayManager] for a single sphere `context`.
    pub async fn new(
        context: C,
        counterpart: Did,
        ipfs_api: Url,
        name_resolver_api: Url,
        cors_origin: Option<Url>,
    ) -> Result<Self> {
        let gateway_identity = context.sphere_context().await?.author().did().await?;
        let gateway_scope = GatewayScope::new(gateway_identity, counterpart);
        let context_resolver =
            SingleTenantContextResolver::new(context.clone(), gateway_scope.clone());
        let job_client = SingleTenantJobClient::new(
            context_resolver.clone(),
            gateway_scope.clone(),
            KuboClient::new(&ipfs_api)?,
            name_resolver_api,
        )
        .await?;
        Ok(SingleTenantGatewayManager {
            context,
            gateway_scope,
            context_resolver,
            job_client,
            ipfs_api,
            cors_origin,
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
    type JobClient = SingleTenantJobClient<C, S>;

    fn job_client(&self) -> Self::JobClient {
        self.job_client.clone()
    }

    fn ipfs_api_url(&self) -> Url {
        self.ipfs_api.to_owned()
    }

    fn cors_origin(&self) -> Option<Url> {
        self.cors_origin.to_owned()
    }

    async fn ucan_store(&self) -> Result<UcanStore<S::BlockStore>> {
        let context = self.context.sphere_context().await?;
        // @TODO CONFIRM
        // We need to be somewhat generic here, we can't have
        // `UcanStore<SphereDb<S>>` for some types of shared ucan stores.
        // @TODO CONFIRM that `.to_block_store()` is functionally
        // equivilent to `UcanStore(context.db())`
        let db = context.db().to_block_store();
        Ok(UcanStore(db))
    }

    async fn sphere_context(&self, counterpart: &Did) -> Result<C> {
        self.context_resolver.get_context(counterpart).await
    }

    async fn gateway_scope(&self, _: &mut Parts) -> Result<(Did, Did), StatusCode> {
        Ok((
            self.gateway_scope.gateway_identity.clone(),
            self.gateway_scope.counterpart.clone(),
        ))
    }
}
