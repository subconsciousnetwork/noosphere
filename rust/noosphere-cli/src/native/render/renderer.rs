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

/// [SphereRenderer] embodies all of the work needed to render a sphere graph to
/// a workspace file system location. Starting from the "root" sphere, the
/// renderer will efficiently queue work to concurrently render the sphere graph
/// up to a maximum depth.
///
/// The renderer produces a file system structure that is approximated by this
/// diagram:
///
/// ```sh
/// /workspace_root/
/// ├── foo.subtext
/// ├── bar/
/// │   └── baz.subtext
/// ├── @my-peer/ -> ./.sphere/peers/bafyabc...x987
/// ├── @other-peer/ -> ./.sphere/peers/bafyabc...y654
/// └── .sphere/
///     ├── identity # The sphere ID of the root sphere
///     ├── version  # Last rendered version of the root sphere
///     ├── depth    # Last rendered depth
///     ├── slugs    # Backward mapping of slugs to files; base64-encoded to escape
///     │   │        # special characters that may occur in a slug (such as '/')
///     │   ├── Zm9v -> ../../foo.subtext
///     │   └── YmFyL2Jheg -> ../../bar/baz.subtext
///     ├── storage/ # Storage folder distinguishes the root sphere
///     │   └── ...  # Implementation-specific e.g., Sled will have its own DB structure
///     ├── content/ # Hard links to content that appears in peer spheres
///     │   ├── bafyabc...a123
///     │   ├── bafyabc...b456
///     │   ├── bafyabc...c789
///     │   └── ...
///     └── peers/
///         ├── bafyabc...x987/
///         │   ├── identity
///         │   ├── version
///         │   ├── link_record  # A valid link record for this peer at this version
///         │   └── mount/       # The virtual root where a peers files an links to thier
///         │       │            # peers are rendered
///         │       ├── their-foo.subtext -> ../../../content/bafyabc...b456
///         │       ├── @peer3 -> ../../../peers/bafyabc...y654/mount
///         │       └── @peer4 -> ../../../peers/bafyabc...z321/mount
///         ├── bafyabc...y654/
///         │   ├── identity
///         │   ├── version
///         │   ├── link_record
///         │   └── mount/
///         │       └── ...
///         ├── bafyabc...z321/
///         │   ├── identity
///         │   ├── version
///         │   ├── link_record
///         │   └── mount/
///         │       └── ...
///         └── ...
/// ```
///
/// Peers throughout the graph are rendered into a flat structure. Each version
/// of a peer sphere gets its own unique directory, and the "mount" subfolder
/// therein contains a virtual file system representation of that sphere's
/// contents and peers. The word "virtual" is used because all content and
/// spheres within the mount are represented as symlinks. This enables maximum
/// re-use of content across revisions of peers over time.
///
/// Note that since peers may re-appear in address books at different depths
/// of graph traversal, it's possible to appear to have rendered more deeply
/// than the "maximum" render depth (when in fact an already-rendered peer is
/// just being re-used).

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
    /// Construct a [SphereRenderer] for the given root [HasSphereContext] and
    /// initialized [SpherePaths].
    pub fn new(context: C, paths: Arc<SpherePaths>) -> Self {
        SphereRenderer {
            context,
            paths,
            storage_type: PhantomData,
        }
    }

    /// Render the sphere graph up to the given depth; the renderer will attempt
    /// to render different edges from the root concurrently, efficiently and
    /// idempotently. If the specified render depth increases for a subsequent
    /// render, all rendered peers will be reset and rendered again (although
    /// the hard links to their content will remain unchanged).
    #[instrument(level = "debug", skip(self))]
    pub async fn render(&self, depth: Option<u32>) -> Result<()> {
        std::env::set_current_dir(self.paths.root())?;

        let mut render_jobs = JoinSet::<Result<()>>::new();
        let mut started_jobs = BTreeSet::<SphereRenderJobId>::new();

        let max_parallel_jobs = available_parallelism()?.get();
        let (tx, mut rx) = tokio::sync::mpsc::channel::<SphereRenderRequest>(max_parallel_jobs);

        let last_render_depth =
            if let Ok(depth) = tokio::fs::read_to_string(self.paths.depth()).await {
                depth.parse::<u32>().ok()
            } else {
                None
            };

        let render_depth = if let Some(depth) = depth {
            depth
        } else {
            last_render_depth.unwrap_or(DEFAULT_RENDER_DEPTH)
        };

        if let Some(last_render_depth) = last_render_depth {
            if render_depth > last_render_depth {
                // NOTE: Sequencing is important here. This reset is performed
                // by the renderer in advance of queuing any work because we
                // cannot guarantee the order in which requests to render peers
                // may come in, and it could happen out-of-order with a "refresh
                // peers" job that is running concurrently.
                self.reset_peers().await?;

                debug!(
                    ?max_parallel_jobs,
                    ?render_depth,
                    "Spawning peer refresh render job for {}...",
                    self.context.identity().await?
                );

                render_jobs.spawn(
                    SphereRenderJob::new(
                        self.context.clone(),
                        JobKind::RefreshPeers,
                        self.paths.clone(),
                        Vec::new(),
                        tx.clone(),
                    )
                    .render(),
                );
            }
        }

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

                            let SphereRenderRequest(petname_path, peer, version, link_record) = job_request;

                            // NOTE: If a peer is too deep, we _don't_ mark it as started; another
                            // peer may wish to render this peer at a shallower depth, in which case
                            // we should proceed.
                            if petname_path.len() > render_depth as usize {
                                debug!("Skipping render job for '@{}' (exceeds max render depth {render_depth})", petname_path.join("."));
                                continue;
                            }
                            warn!("PETNAME PATH {:?}", petname_path);

                            debug!("Queuing render job for {} @ {}...", job_id.0, job_id.1);
                            started_jobs.insert(job_id);

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

    async fn reset_peers(&self) -> Result<()> {
        if let Err(error) = tokio::fs::remove_dir_all(self.paths.peers()).await {
            warn!(
                path = ?self.paths.peers(),
                "Failed attempt to reset peers: {}", error
            );
        }

        tokio::fs::create_dir_all(self.paths.peers()).await?;

        Ok(())
    }
}
