use crate::ContextResolver;
use noosphere_core::context::HasMutableSphereContext;
use noosphere_ipfs::IpfsClient;
use noosphere_ns::NameResolver;
use noosphere_storage::Storage;
use std::marker::PhantomData;

/// Context provided to processors in order to resolve additional
/// resources needed to process jobs.
#[derive(Clone)]
pub struct GatewayJobContext<R, C, S, N, I>
where
    Self: Send,
    R: ContextResolver<C, S>,
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
    N: NameResolver + Clone,
    I: IpfsClient + Send + Sync,
{
    /// [ContextResolver] associated with this job processor.
    pub context_resolver: R,
    /// [NameResolver] associated with this job processor.
    pub name_resolver: N,
    /// [IpfsClient] associated with this job processor.
    pub ipfs_client: I,
    context_marker: PhantomData<C>,
    storage_marker: PhantomData<S>,
}

impl<R, C, S, N, I> GatewayJobContext<R, C, S, N, I>
where
    R: ContextResolver<C, S>,
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
    N: NameResolver + Clone,
    I: IpfsClient + Send + Sync,
{
    /// Creates a new [GatewayJobContext].
    pub fn new(context_resolver: R, name_resolver: N, ipfs_client: I) -> Self {
        Self {
            context_resolver,
            name_resolver,
            ipfs_client,
            context_marker: PhantomData,
            storage_marker: PhantomData,
        }
    }
}
