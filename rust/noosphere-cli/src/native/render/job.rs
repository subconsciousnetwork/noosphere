use anyhow::{anyhow, Result};
use cid::Cid;
use noosphere_core::data::{Did, Link, MemoIpld};
use noosphere_sphere::{
    HasSphereContext, SphereContentRead, SphereCursor, SpherePetnameRead, SphereReplicaRead,
    SphereWalker,
};
use noosphere_storage::Storage;
use std::marker::PhantomData;
use tokio::sync::mpsc::Sender;
use tokio_stream::StreamExt;

use crate::native::{paths::SpherePaths, render::ChangeBuffer};

use super::writer::SphereWriter;

const CONTENT_CHANGE_BUFFER_CAPACITY: usize = 512;
const PETNAME_CHANGE_BUFFER_CAPACITY: usize = 2048;

pub struct SphereRenderJob<C, S>
where
    C: HasSphereContext<S> + 'static,
    S: Storage + 'static,
{
    pub context: C,
    pub petname_path: Vec<String>,
    pub writer: SphereWriter,
    pub storage_type: PhantomData<S>,
    pub job_queue: Sender<SphereRenderJobId>,
}

impl<C, S> SphereRenderJob<C, S>
where
    C: HasSphereContext<S> + 'static,
    S: Storage + 'static,
{
    fn paths(&self) -> &SpherePaths {
        self.writer.paths()
    }

    #[instrument(level = "debug", skip(self))]
    pub async fn render(self) -> Result<()> {
        if self.writer.is_root_writer() {
            debug!("Running root render job...");
            match tokio::fs::try_exists(self.paths().version()).await {
                Ok(true) => {
                    debug!("Root has already been rendering; rendering incrementally...");
                    let version =
                        Cid::try_from(tokio::fs::read_to_string(self.paths().version()).await?)?;
                    self.incremental_render(&version.into()).await?;
                }
                _ => {
                    debug!("Root has not been rendered yet; performing a full render...");
                    self.full_render(SphereCursor::latest(self.context.clone()))
                        .await?
                }
            }
        } else {
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

        Ok(())
    }

    #[instrument(level = "debug", skip(self, cursor))]
    async fn full_render(&self, cursor: SphereCursor<C, S>) -> Result<()> {
        let identity = cursor.identity().await?;
        let version = cursor.version().await?;

        debug!("Starting full render of {identity} @ {version}...");

        let walker = SphereWalker::from(&cursor);

        let content_stream = walker.content_stream();
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

        let petname_stream = walker.petname_stream();
        let db = cursor.sphere_context().await?.db().clone();

        let mut petname_change_buffer = ChangeBuffer::new(PETNAME_CHANGE_BUFFER_CAPACITY);

        tokio::pin!(petname_stream);

        // Write all peer symlinks, queuing jobs to render them as we go
        while let Some((name, identity)) = petname_stream.try_next().await? {
            let did = identity.did.clone();
            let cid = match identity.link_record(&db).await {
                None => None,
                Some(link_record) => link_record.get_link(),
            };
            let cid = match cid {
                Some(cid) => cid,
                None => {
                    warn!("No version resolved for '@{name}', skipping...");
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
            self.job_queue.send((petname_path, did, cid.into())).await?;
        }

        petname_change_buffer.flush_to_writer(&self.writer).await?;

        // Write out the latest version that was rendered
        tokio::fs::write(self.paths().version(), version.to_string()).await?;

        Ok(())
    }

    #[instrument(level = "debug", skip(self))]
    async fn incremental_render(&self, since: &Link<MemoIpld>) -> Result<()> {
        let walker = SphereWalker::from(&self.context);
        let content_change_stream = walker.content_change_stream(Some(since));
        let mut latest_version = None;
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

            latest_version = Some(version);
        }

        content_change_buffer.flush_to_writer(&self.writer).await?;

        let petname_change_stream = walker.petname_change_stream(Some(since));
        let mut petname_change_buffer = ChangeBuffer::new(PETNAME_CHANGE_BUFFER_CAPACITY);

        tokio::pin!(petname_change_stream);

        while let Some((version, changes)) = petname_change_stream.try_next().await? {
            cursor.mount_at(&version).await?;

            for petname in changes {
                match cursor.get_petname(&petname).await? {
                    Some(identity) => match cursor.resolve_petname(&petname).await? {
                        Some(version) => {
                            petname_change_buffer.add(
                                petname.clone(),
                                (identity.clone(), Cid::from(version.clone())),
                            )?;

                            let mut petname_path = self.petname_path.clone();
                            petname_path.push(petname);
                            self.job_queue
                                .send((petname_path, identity, Cid::from(version)))
                                .await?;
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

        if let Some(version) = latest_version {
            tokio::fs::write(self.paths().version(), version.to_string()).await?;
        }

        Ok(())
    }
}

pub type SphereRenderJobId = (Vec<String>, Did, Cid);
