use anyhow::Result;
use noosphere_storage::Storage;

use ucan::crypto::KeyMaterial;

use crate::HasSphereContext;
use async_trait::async_trait;

use crate::{internal::SphereContextInternal, AsyncFileBody, SphereFile};

/// Anything that can read content from a sphere should implement [SphereContentRead].
/// A blanket implementation is provided for anything that implements [HasSphereContext].
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait SphereContentRead<K, S>
where
    K: KeyMaterial + Clone + 'static,
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
impl<H, K, S> SphereContentRead<K, S> for H
where
    H: HasSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    async fn read(&self, slug: &str) -> Result<Option<SphereFile<Box<dyn AsyncFileBody>>>> {
        let revision = self.version().await?;
        let sphere = self.to_sphere().await?;

        let links = sphere.get_links().await?;
        let hamt = links.get_hamt().await?;

        Ok(match hamt.get(&slug.to_string()).await? {
            Some(content_cid) => Some(self.get_file(&revision, content_cid).await?),
            None => None,
        })
    }
}
