use anyhow::Result;
use noosphere_storage::Storage;

use crate::context::HasSphereContext;
use async_trait::async_trait;

use crate::context::{internal::SphereContextInternal, AsyncFileBody, SphereFile};

/// Anything that can read content from a sphere should implement [SphereContentRead].
/// A blanket implementation is provided for anything that implements [HasSphereContext].
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait SphereContentRead<S>
where
    S: Storage + 'static,
{
    /// Read a file that is associated with a given slug at the revision of the
    /// sphere that this view is pointing to.
    /// Note that "contents" are `AsyncRead`, and content bytes won't be read
    /// until contents is polled.
    async fn read(&self, slug: &str) -> Result<Option<SphereFile<Box<dyn AsyncFileBody>>>>;

    /// Returns true if the content identitifed by slug exists in the sphere at
    /// the current revision.
    async fn exists(&self, slug: &str) -> Result<bool> {
        Ok(self.read(slug).await?.is_some())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<C, S> SphereContentRead<S> for C
where
    C: HasSphereContext<S>,
    S: Storage + 'static,
{
    async fn read(&self, slug: &str) -> Result<Option<SphereFile<Box<dyn AsyncFileBody>>>> {
        let revision = self.version().await?;
        let sphere = self.to_sphere().await?;

        let links = sphere.get_content().await?;
        let hamt = links.get_hamt().await?;

        Ok(match hamt.get(&slug.to_string()).await? {
            Some(memo) => Some(self.get_file(&revision, *memo).await?),
            None => None,
        })
    }
}
