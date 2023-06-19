use anyhow::Result;
use cid::Cid;
use noosphere_core::data::Did;
use noosphere_sphere::{HasSphereContext, SphereWalker};
use noosphere_storage::Storage;
use std::{collections::BTreeMap, marker::PhantomData};
use tokio_stream::StreamExt;

use super::paths::{SpherePaths, SphereWriter};

pub struct SphereRenderJob<'a, C, S>
where
    Self: 'a,
    C: HasSphereContext<S>,
    S: Storage + 'static,
{
    context: C,
    writer: SphereWriter<'a>,
    storage_type: PhantomData<S>,
}

impl<'a, C, S> SphereRenderJob<'a, C, S>
where
    Self: 'a,
    C: HasSphereContext<S>,
    S: Storage + 'static,
{
    pub async fn render(&self) -> Result<()> {
        let walker = SphereWalker::from(&self.context);

        let content_stream = walker.content_stream();
        tokio::pin!(content_stream);

        while let Some((slug, mut file)) = content_stream.try_next().await? {
            self.writer.write_content(&slug, &mut file).await?;
        }

        Ok(())
    }
}

pub type SphereRenderJobId = (Did, Cid);

pub struct SphereRenderer<'a, C, S>
where
    Self: 'a,
    C: HasSphereContext<S>,
    S: Storage + 'static,
{
    context: C,
    writer: SphereWriter<'a>,
    paths: SpherePaths,
    peer_render_jobs: BTreeMap<SphereRenderJobId, SphereRenderJob<'a, C, S>>,
    storage_type: PhantomData<S>,
}

impl<'a, C, S> SphereRenderer<'a, C, S>
where
    Self: 'a,
    C: HasSphereContext<S>,
    S: Storage + 'static,
{
    pub async fn render(&self) -> Result<()> {
        let walker = SphereWalker::from(&self.context);

        let content_stream = walker.content_stream();
        tokio::pin!(content_stream);

        while let Some((slug, mut file)) = content_stream.try_next().await? {
            self.writer.write_content(&slug, &mut file).await?;
        }

        Ok(())
    }
}
