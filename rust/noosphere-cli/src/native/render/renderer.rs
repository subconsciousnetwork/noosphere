use anyhow::Result;
use noosphere_sphere::HasSphereContext;
use noosphere_storage::Storage;
use std::{collections::BTreeSet, marker::PhantomData, sync::Arc, thread::available_parallelism};
use tokio::{select, task::JoinSet};

use super::{SphereRenderJob, SphereRenderRequest};
use crate::native::{
    paths::SpherePaths,
    render::{JobKind, SphereRenderJobId},
};

const DEFAULT_RENDER_DEPTH: u32 = 3;

pub struct SphereRenderer<C, S>
where
    C: HasSphereContext<S> + 'static,
    S: Storage + 'static,
{
    context: C,
    paths: Arc<SpherePaths>,
    storage_type: PhantomData<S>,
}

impl<C, S> SphereRenderer<C, S>
where
    C: HasSphereContext<S> + 'static,
    S: Storage + 'static,
{
    pub fn new(context: C, paths: Arc<SpherePaths>) -> Self {
        SphereRenderer {
            context,
            paths,
            storage_type: PhantomData,
        }
    }

    #[instrument(level = "debug", skip(self))]
    pub async fn render(&self, depth: Option<u32>) -> Result<()> {
        std::env::set_current_dir(self.paths.root())?;

        let mut render_jobs = JoinSet::<Result<()>>::new();
        let mut started_jobs = BTreeSet::<SphereRenderJobId>::new();

        let max_parallel_jobs = available_parallelism()?.get();
        let (tx, mut rx) = tokio::sync::mpsc::channel::<SphereRenderRequest>(max_parallel_jobs);

        let render_depth = if let Some(depth) = depth {
            depth
        } else if let Ok(depth) = tokio::fs::read_to_string(self.paths.depth()).await {
            depth.parse::<u32>().unwrap_or(DEFAULT_RENDER_DEPTH)
        } else {
            DEFAULT_RENDER_DEPTH
        };

        debug!(
            ?max_parallel_jobs,
            ?render_depth,
            "Spawning root render job for {}...",
            self.context.identity().await?
        );

        // Spawn the root job
        render_jobs.spawn(
            SphereRenderJob::new(
                self.context.clone(),
                JobKind::Root,
                self.paths.clone(),
                Vec::new(),
                tx.clone(),
            )
            .render(),
        );

        let mut job_queue_open = true;

        while !render_jobs.is_empty() && job_queue_open {
            select! {
                result = render_jobs.join_next() => {
                    if let Some(result) = result {
                        result??;
                    }
                },
                next_job_request = rx.recv() => {
                    match next_job_request {
                        None => {
                            job_queue_open = false;
                        }
                        Some(job_request) => {
                            let job_id = job_request.as_id();

                            if started_jobs.contains(&job_id) {
                                debug!("A render job for {} @ {} has already been queued, skipping...", job_id.0, job_id.1);
                                continue;
                            }

                            debug!("Queuing render job for {} @ {}...", job_id.0, job_id.1);
                            started_jobs.insert(job_id);

                            let SphereRenderRequest(petname_path, peer, version, link_record) = job_request;

                            if petname_path.len() > render_depth as usize {
                                debug!("Skipping '@{}' (exceeds max render depth)", petname_path.join("."));
                                continue;
                            }

                            if self.paths.peer(&peer, &version).exists() {
                                // TODO: We may need to re-render if a previous
                                // render was incomplete for some reason
                                debug!(
                                    "Content for {} @ {} is already rendered, skipping...",
                                    peer, version
                                );
                                continue;
                            }

                            debug!("Spawning render job for {peer} @ {version}...");

                            render_jobs.spawn(
                                SphereRenderJob::new(
                                    self.context.clone(),
                                    JobKind::Peer(peer, version, link_record),
                                    self.paths.clone(),
                                    petname_path,
                                    tx.clone()
                                ).render()
                            );
                        }
                    }
                }
            }
        }

        tokio::fs::write(self.paths.depth(), render_depth.to_string()).await?;

        Ok(())
    }
}
