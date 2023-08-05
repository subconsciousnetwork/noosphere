//! Constructs related to performing specific rendering tasks

use anyhow::{anyhow, Result};
use cid::Cid;
use noosphere_core::data::{Did, Link, LinkRecord, MemoIpld};
use noosphere_sphere::{
    HasSphereContext, SphereContentRead, SphereCursor, SpherePetnameRead, SphereReplicaRead,
    SphereWalker,
};
use noosphere_storage::Storage;
use std::{marker::PhantomData, sync::Arc};
use tokio::sync::mpsc::Sender;
use tokio_stream::StreamExt;

use crate::native::{paths::SpherePaths, render::ChangeBuffer};

use super::writer::SphereWriter;

const CONTENT_CHANGE_BUFFER_CAPACITY: usize = 512;
const PETNAME_CHANGE_BUFFER_CAPACITY: usize = 2048;

/// A pairing of [Did] and [Cid], suitable for uniquely identifying the work
/// needed to render a specific sphere at a specific version, regardless of
/// relative location in the graph
pub type SphereRenderJobId = (Did, Cid);

/// A request to render a sphere, originating from a given path through the
/// graph.
pub struct SphereRenderRequest(pub Vec<String>, pub Did, pub Cid, pub LinkRecord);

impl SphereRenderRequest {
    /// Get the [SphereRenderJobId] that corresponds to this request
    pub fn as_id(&self) -> SphereRenderJobId {
        (self.1.clone(), self.2)
    }
}

/// The kind of render work to be performed by a [SphereRenderJob]. The effects
/// of using each variant have a lot of overlap, but the specific results vary
/// significantly
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JobKind {
    /// A job that renders the root sphere
    Root {
        /// If an incremental render would be performed, force a full render
        force_full_render: bool,
    },
    /// A job that renders a peer (or peer-of-a-peer) of the root sphere
    Peer(Did, Cid, LinkRecord),
    /// A job that renders _just_ the peers of the root sphere
    RefreshPeers,
}

/// A [SphereRenderJob] encapsulates the state needed to perform some discrete
/// [JobKind], and implements the specified render path that corresponds to that
/// [JobKind]. It is designed to be able to be run concurrently or in parallel
/// with an arbitrary number of other [SphereRenderJob]s that are associated with
/// the local sphere workspace.
pub struct SphereRenderJob<C, S>
where
    C: HasSphereContext<S> + 'static,
    S: Storage + 'static,
{
    context: C,
    kind: JobKind,
    petname_path: Vec<String>,
    writer: SphereWriter,
    storage_type: PhantomData<S>,
    job_queue: Sender<SphereRenderRequest>,
}

impl<C, S> SphereRenderJob<C, S>
where
    C: HasSphereContext<S> + 'static,
    S: Storage + 'static,
{
    /// Construct a new render job of [JobKind], using the given [HasSphereContext] and
    /// [SpherePaths] to perform rendering.
    pub fn new(
        context: C,
        kind: JobKind,
        paths: Arc<SpherePaths>,
        petname_path: Vec<String>,
        job_queue: Sender<SphereRenderRequest>,
    ) -> Self {
        SphereRenderJob {
            context,
            petname_path,
            writer: SphereWriter::new(kind.clone(), paths),
            kind,
            storage_type: PhantomData,
            job_queue,
        }
    }

    /// Entrypoint to render based on the [SphereRenderJob] configuration
    #[instrument(level = "debug", skip(self))]
    pub async fn render(self) -> Result<()> {
        match self.kind {
            JobKind::Root { force_full_render } => {
                debug!("Running root render job...");
                match (
                    force_full_render,
                    tokio::fs::try_exists(self.paths().version()).await,
                ) {
                    (false, Ok(true)) => {
                        debug!("Root has been rendered at least once; rendering incrementally...");
                        let version = Cid::try_from(
                            tokio::fs::read_to_string(self.paths().version()).await?,
                        )?;
                        self.incremental_render(&version.into()).await?;
                    }
                    _ => {
                        if force_full_render {
                            debug!("Root full render is being forced...");
                        } else {
                            debug!("Root has not been rendered yet; performing a full render...");
                        }
                        self.full_render(SphereCursor::latest(self.context.clone()))
                            .await?
                    }
                }
            }
            JobKind::Peer(_, _, _) => {
                debug!("Running peer render job...");
                if let Some(context) = SphereCursor::latest(self.context.clone())
                    .traverse_by_petnames(&self.petname_path)
                    .await?
                {
                    self.full_render(context).await?;
                } else {
                    return Err(anyhow!("No peer found at {}", self.petname_path.join(".")));
                };
            }
            JobKind::RefreshPeers => {
                debug!("Running refresh peers render job...");
                self.refresh_peers(SphereCursor::latest(self.context.clone()))
                    .await?;
            }
        }

        Ok(())
    }

    fn paths(&self) -> &SpherePaths {
        self.writer.paths()
    }

    #[instrument(level = "debug", skip(self, cursor))]
    async fn full_render(&self, cursor: SphereCursor<C, S>) -> Result<()> {
        let identity = cursor.identity().await?;
        let version = cursor.version().await?;

        debug!("Starting full render of {identity} @ {version}...");

        {
            let content_stream = SphereWalker::from(&cursor).into_content_stream();

            tokio::pin!(content_stream);

            let mut content_change_buffer = ChangeBuffer::new(CONTENT_CHANGE_BUFFER_CAPACITY);

            // Write all content
            while let Some((slug, file)) = content_stream.try_next().await? {
                content_change_buffer.add(slug, file)?;

                if content_change_buffer.is_full() {
                    content_change_buffer.flush_to_writer(&self.writer).await?;
                }
            }

            content_change_buffer.flush_to_writer(&self.writer).await?;
        }

        self.refresh_peers(cursor).await?;

        // Write out the latest version that was rendered
        tokio::try_join!(
            self.writer.write_identity_and_version(&identity, &version),
            self.writer.write_link_record()
        )?;

        Ok(())
    }

    #[instrument(level = "debug", skip(self, cursor))]
    async fn refresh_peers(&self, cursor: SphereCursor<C, S>) -> Result<()> {
        let petname_stream = SphereWalker::from(&cursor).into_petname_stream();
        let db = cursor.sphere_context().await?.db().clone();

        let mut petname_change_buffer = ChangeBuffer::new(PETNAME_CHANGE_BUFFER_CAPACITY);

        tokio::pin!(petname_stream);

        // Write all peer symlinks, queuing jobs to render them as we go
        while let Some((name, identity)) = petname_stream.try_next().await? {
            let did = identity.did.clone();
            let (link_record, cid) = match identity.link_record(&db).await {
                Some(link_record) => {
                    if let Some(cid) = link_record.get_link() {
                        (link_record, cid)
                    } else {
                        warn!("No version resolved for '@{name}', skipping...");
                        continue;
                    }
                }
                None => {
                    warn!("No link record found for '@{name}', skipping...");
                    continue;
                }
            };

            // Create a symlink to each peer (they will be rendered later, if
            // they haven't been already)
            petname_change_buffer.add(name.clone(), (did.clone(), cid.clone().into()))?;

            if petname_change_buffer.is_full() {
                petname_change_buffer.flush_to_writer(&self.writer).await?;
            }

            // Ensure the peer is queued to be rendered (redundant jobs are
            // depuplicated by the receiver)
            let mut petname_path = vec![name];
            petname_path.append(&mut self.petname_path.clone());
            self.job_queue
                .send(SphereRenderRequest(
                    petname_path,
                    did,
                    cid.into(),
                    link_record,
                ))
                .await?;
        }

        petname_change_buffer.flush_to_writer(&self.writer).await?;

        Ok(())
    }

    #[instrument(level = "debug", skip(self))]
    async fn incremental_render(&self, since: &Link<MemoIpld>) -> Result<()> {
        let content_change_stream =
            SphereWalker::from(&self.context).into_content_change_stream(Some(since));
        let mut cursor = SphereCursor::latest(self.context.clone());
        let mut content_change_buffer = ChangeBuffer::new(CONTENT_CHANGE_BUFFER_CAPACITY);

        tokio::pin!(content_change_stream);

        while let Some((version, changes)) = content_change_stream.try_next().await? {
            cursor.mount_at(&version).await?;

            for slug in changes {
                trace!(version = ?version, slug = ?slug, "Buffering change...");
                match cursor.read(&slug).await? {
                    Some(file) => content_change_buffer.add(slug, file)?,
                    None => content_change_buffer.remove(&slug)?,
                }

                if content_change_buffer.is_full() {
                    content_change_buffer.flush_to_writer(&self.writer).await?;
                }
            }
        }

        content_change_buffer.flush_to_writer(&self.writer).await?;

        let petname_change_stream =
            SphereWalker::from(&self.context).into_petname_change_stream(Some(since));
        let mut petname_change_buffer = ChangeBuffer::new(PETNAME_CHANGE_BUFFER_CAPACITY);

        tokio::pin!(petname_change_stream);

        while let Some((version, changes)) = petname_change_stream.try_next().await? {
            cursor.mount_at(&version).await?;

            for petname in changes {
                match cursor.get_petname(&petname).await? {
                    Some(identity) => match cursor.get_petname_record(&petname).await? {
                        Some(link_record) => {
                            if let Some(version) = link_record.get_link() {
                                petname_change_buffer.add(
                                    petname.clone(),
                                    (identity.clone(), Cid::from(version.clone())),
                                )?;

                                let mut petname_path = self.petname_path.clone();
                                petname_path.push(petname);
                                self.job_queue
                                    .send(SphereRenderRequest(
                                        petname_path,
                                        identity,
                                        Cid::from(version),
                                        link_record,
                                    ))
                                    .await?;
                            } else {
                                petname_change_buffer.remove(&petname)?;
                            }
                        }
                        None => petname_change_buffer.remove(&petname)?,
                    },
                    None => petname_change_buffer.remove(&petname)?,
                }

                if petname_change_buffer.is_full() {
                    petname_change_buffer.flush_to_writer(&self.writer).await?;
                }
            }
        }

        petname_change_buffer.flush_to_writer(&self.writer).await?;

        // Write out the latest version that was rendered
        let identity = cursor.identity().await?;
        let version = cursor.version().await?;

        self.writer
            .write_identity_and_version(&identity, &version)
            .await?;

        Ok(())
    }
}
