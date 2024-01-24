use crate::{
    jobs::{processors::*, worker_queue::Processor, GatewayJob, GatewayJobContext},
    ContextResolver,
};
use anyhow::Result;
use async_trait::async_trait;
use noosphere_core::context::HasMutableSphereContext;
use noosphere_ipfs::IpfsClient;
use noosphere_ns::NameResolver;
use noosphere_storage::Storage;
use std::marker::PhantomData;

/// Implements [Processor] for [GatewayJob] tasks.
#[derive(Clone)]
pub struct GatewayJobProcessor<R, C, S, N, I>
where
    R: ContextResolver<C, S>,
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
    N: NameResolver + Clone,
    I: IpfsClient + Send + Sync,
{
    context_resolver_marker: PhantomData<R>,
    name_resolver_marker: PhantomData<N>,
    ipfs_client_marker: PhantomData<I>,
    context_marker: PhantomData<C>,
    storage_marker: PhantomData<S>,
}

#[async_trait]
impl<R, C, S, N, I> Processor for GatewayJobProcessor<R, C, S, N, I>
where
    R: ContextResolver<C, S> + 'static,
    C: HasMutableSphereContext<S> + 'static,
    S: Storage + 'static,
    N: NameResolver + Clone + 'static,
    I: IpfsClient + Send + Sync + 'static,
{
    type Context = GatewayJobContext<R, C, S, N, I>;
    type Job = GatewayJob;

    async fn process(context: Self::Context, job: Self::Job) -> Result<Option<Self::Job>> {
        process_job(context, job).await
    }
}

/// Performs the work in current thread to complete a [GatewayJob].
pub async fn process_job<R, C, S, N, I>(
    context: GatewayJobContext<R, C, S, N, I>,
    job: GatewayJob,
) -> Result<Option<GatewayJob>>
where
    R: ContextResolver<C, S>,
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
    N: NameResolver + Clone + 'static,
    I: IpfsClient + Send + Sync + 'static,
{
    match job {
        GatewayJob::CompactHistory { identity } => {
            compact_sphere(context.context_resolver.get_context(&identity).await?).await
        }
        GatewayJob::IpfsSyndication {
            identity,
            revision,
            name_publish_on_success,
        } => {
            syndicate_to_ipfs(
                context.context_resolver.get_context(&identity).await?,
                revision,
                context.ipfs_client,
                name_publish_on_success,
            )
            .await
        }
        GatewayJob::NameSystemResolveAll { identity } => {
            name_system_resolve_all(
                context.context_resolver.get_context(&identity).await?,
                context.ipfs_client,
                context.name_resolver,
            )
            .await
        }
        GatewayJob::NameSystemResolveSince { identity, since } => {
            name_system_resolve_since(
                context.context_resolver.get_context(&identity).await?,
                context.ipfs_client,
                context.name_resolver,
                since,
            )
            .await
        }
        GatewayJob::NameSystemPublish { identity, record } => {
            name_system_publish(
                context.context_resolver.get_context(&identity).await?,
                context.name_resolver,
                record,
            )
            .await
        }
        GatewayJob::NameSystemRepublish { identity } => {
            name_system_republish(
                context.context_resolver.get_context(&identity).await?,
                context.name_resolver,
            )
            .await
        }
    }
}
