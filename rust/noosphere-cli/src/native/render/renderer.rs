use anyhow::Result;
use noosphere_sphere::HasSphereContext;
use noosphere_storage::Storage;
use std::{collections::BTreeSet, marker::PhantomData, sync::Arc, thread::available_parallelism};
use tokio::{select, task::JoinSet};

use super::{SphereRenderJob, SphereRenderJobId};
use crate::native::{paths::SpherePaths, render::JobKind};

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

    pub async fn render(&self) -> Result<()> {
        std::env::set_current_dir(self.paths.root())?;

        let mut render_jobs = JoinSet::<Result<()>>::new();
        let mut started_jobs = BTreeSet::<SphereRenderJobId>::new();

        let max_parallel_jobs = available_parallelism()?.get();
        let (tx, mut rx) = tokio::sync::mpsc::channel::<SphereRenderJobId>(max_parallel_jobs);

        // let root_writer = SphereWriter::new(self.paths.clone());

        debug!(
            ?max_parallel_jobs,
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
                next_job = rx.recv() => {
                    match next_job {
                        None => {
                            job_queue_open = false;
                        }
                        Some(job_id) => {
                            if started_jobs.contains(&job_id) {
                                continue;
                            }

                            started_jobs.insert(job_id.clone());

                            let (petname_path, peer, version) = job_id;

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
                                    JobKind::Peer(peer, version),
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
        Ok(())
    }
}
