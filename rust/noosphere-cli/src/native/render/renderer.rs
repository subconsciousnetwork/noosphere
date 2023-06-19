use anyhow::Result;
use noosphere_sphere::HasSphereContext;
use noosphere_storage::Storage;
use std::{collections::BTreeSet, marker::PhantomData, sync::Arc, thread::available_parallelism};
use tokio::task::JoinSet;

use super::{writer::SphereWriter, SphereRenderJob, SphereRenderJobId};
use crate::native::paths::SpherePaths;

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

        let root_writer = SphereWriter::new(self.paths.clone());

        debug!(
            "Spawning root render job for {}...",
            self.context.identity().await?
        );

        trace!("Maximum parallel render jobs: {max_parallel_jobs}");

        // Spawn the root job
        render_jobs.spawn(
            SphereRenderJob {
                context: self.context.clone(),
                petname_path: Vec::new(),
                writer: root_writer.clone(),
                storage_type: PhantomData,
                job_queue: tx.clone(),
            }
            .render(),
        );

        while let Some(result) = render_jobs.join_next().await {
            result??;

            while let Ok(job_id) = rx.try_recv() {
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
                    SphereRenderJob {
                        context: self.context.clone(),
                        petname_path,
                        writer: root_writer.descend(&peer, &version),
                        storage_type: PhantomData,
                        job_queue: tx.clone(),
                    }
                    .render(),
                );
            }
        }
        Ok(())
    }
}
