use crate::{
    extractors::GatewayScope,
    jobs::{
        worker_queue::{WorkerQueue, WorkerQueueBuilder},
        GatewayJob, GatewayJobContext, GatewayJobProcessor, JobClient,
    },
    SingleTenantContextResolver,
};
use anyhow::Result;
use noosphere_core::context::HasMutableSphereContext;
use noosphere_ipfs::KuboClient;
use noosphere_ns::server::HttpClient as NameSystemHttpClient;
use noosphere_storage::Storage;
use std::{marker::PhantomData, sync::Arc, time::Duration};
use tokio::task::JoinHandle;
use url::Url;

#[cfg(doc)]
use crate::single_tenant::SingleTenantGatewayManager;

/// The number of worker threads to process jobs.
const WORKER_COUNT: usize = 5;
/// How many times by default jobs should retry upon failure.
/// @TODO Consider allowing individual jobs to hint if they
/// should be retried, versus failures that can be attempted
/// on next periodic cycle.
const JOB_RETRIES: usize = 1;
/// How many seconds before a job is considered broken,
/// and potentially restarted.
const TIMEOUT_SECONDS: u64 = 180;

type GatewayWorkerQueue<C, S> = Arc<
    WorkerQueue<
        GatewayJobProcessor<
            SingleTenantContextResolver<C, S>,
            C,
            S,
            NameSystemHttpClient,
            KuboClient,
        >,
    >,
>;

/// [JobClient] implementation for [SingleTenantGatewayManager].
pub struct SingleTenantJobClient<C, S>
where
    C: HasMutableSphereContext<S> + 'static,
    S: Storage + 'static,
{
    worker_queue: GatewayWorkerQueue<C, S>,
    scheduler_handle: JoinHandle<Result<()>>,
    sphere_context_marker: PhantomData<C>,
    storage_marker: PhantomData<S>,
}

impl<C, S> SingleTenantJobClient<C, S>
where
    C: HasMutableSphereContext<S> + 'static,
    S: Storage + 'static,
{
    /// Creates a new [SingleTenantJobClient].
    pub async fn new(
        context_resolver: SingleTenantContextResolver<C, S>,
        gateway_scope: GatewayScope<C, S>,
        ipfs_client: KuboClient,
        name_resolver_api: Url,
    ) -> Result<Self> {
        let name_resolver = NameSystemHttpClient::new(name_resolver_api).await?;
        let worker_context = GatewayJobContext::new(context_resolver, name_resolver, ipfs_client);
        let worker_queue = Arc::new(
            WorkerQueueBuilder::new()
                .with_worker_count(WORKER_COUNT)
                .with_context(worker_context)
                .with_retries(JOB_RETRIES)
                .with_timeout(Duration::from_secs(TIMEOUT_SECONDS))
                .build()?,
        );

        let scheduler_queue = worker_queue.clone();
        let scheduler_handle =
            tokio::spawn(
                async move { run_periodic_scheduler(gateway_scope, scheduler_queue).await },
            );

        Ok(Self {
            scheduler_handle,
            worker_queue,
            sphere_context_marker: PhantomData,
            storage_marker: PhantomData,
        })
    }
}

impl<C, S> JobClient for SingleTenantJobClient<C, S>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    fn submit(&self, job: GatewayJob) -> Result<()> {
        self.worker_queue.submit(job)
    }
}

impl<C, S> Drop for SingleTenantJobClient<C, S>
where
    C: HasMutableSphereContext<S> + 'static,
    S: Storage + 'static,
{
    fn drop(&mut self) {
        self.scheduler_handle.abort();
    }
}

/// Submits `job` every `seconds` seconds to the provided `queue`.
async fn schedule<C, S>(queue: &GatewayWorkerQueue<C, S>, job: GatewayJob, seconds: u64)
where
    C: HasMutableSphereContext<S> + 'static,
    S: Storage + 'static,
{
    let duration = Duration::from_secs(seconds);
    loop {
        let _ = queue
            .submit(job.clone())
            .map_err(|e| error!("Failed to submit periodic job: {:#?} : {}", &job, e));
        tokio::time::sleep(duration).await
    }
}

/// A non-terminating function that continuously schedules [GatewayJob]s
/// to be processed via a [WorkerQueue].
async fn run_periodic_scheduler<C, S>(
    scope: GatewayScope<C, S>,
    queue: GatewayWorkerQueue<C, S>,
) -> Result<()>
where
    C: HasMutableSphereContext<S> + 'static,
    S: Storage + 'static,
{
    let identity = scope.gateway;
    let _ = tokio::join!(
        schedule(
            &queue,
            GatewayJob::CompactHistory {
                identity: identity.clone()
            },
            60 * 60, // 1 hour
        ),
        schedule(
            &queue,
            GatewayJob::NameSystemResolveAll {
                identity: identity.clone()
            },
            60, // 1 minute
        ),
        schedule(
            &queue,
            GatewayJob::IpfsSyndication {
                identity: identity.clone(),
                name_publish_on_success: None,
            },
            60 * 5, // 5 minutes
        ),
        schedule(
            &queue,
            GatewayJob::NameSystemRepublish {
                identity: identity.clone(),
            },
            60 * 5, // 5 minutes
        )
    );
    Ok(())
}
