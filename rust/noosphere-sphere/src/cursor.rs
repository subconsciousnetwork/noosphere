use anyhow::Result;
use async_trait::async_trait;
use cid::Cid;
use noosphere_storage::Storage;
use ucan::crypto::KeyMaterial;

use crate::{HasMutableSphereContext, HasSphereContext};
use std::marker::PhantomData;

#[cfg(doc)]
use crate::SphereContext;

/// A [SphereCursor] is a structure that enables reading from and writing to a
/// [SphereContext] at specific versions of the associated sphere's history.
/// There are times when you may wish to be able to use the convenience
/// implementation of traits built on [HasSphereContext], but to always be sure
/// of what version you are using them on (such as when traversing sphere
/// history). That is when you would use a [SphereCursor], which can wrap any
/// implementor of [HasSphereContext] and mount it to a specific version of the
/// sphere.
#[derive(Clone)]
pub struct SphereCursor<C, K, S>
where
    C: HasSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    has_sphere_context: C,
    key: PhantomData<K>,
    storage: PhantomData<S>,
    sphere_version: Option<Cid>,
}

impl<C, K, S> SphereCursor<C, K, S>
where
    C: HasSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    /// Same as [SphereCursor::mount], but mounts the [SphereCursor] to a known
    /// version of the history of the sphere.
    pub fn mounted_at(has_sphere_context: C, sphere_version: &Cid) -> Self {
        SphereCursor {
            has_sphere_context,
            key: PhantomData,
            storage: PhantomData,
            sphere_version: Some(*sphere_version),
        }
    }

    /// "Mount" the [SphereCursor] to the latest local version of the sphere it
    /// refers to. If the [SphereCursor] is already mounted, the version it is
    /// mounted to will be overwritten. A mounted [SphereCursor] will remain at
    /// the version it is mounted to even when the latest version of the sphere
    /// changes.
    pub async fn mount<'a>(&'a mut self) -> Result<&'a Self> {
        let sphere_version = self
            .has_sphere_context
            .sphere_context()
            .await?
            .head()
            .await?;

        self.sphere_version = Some(sphere_version);

        Ok(self)
    }

    /// "Unmount" the [SphereCursor] so that it always uses the latest local
    /// version of the sphere that it refers to.
    pub fn unmount(mut self) -> Result<Self> {
        self.sphere_version = None;
        Ok(self)
    }

    /// Create the [SphereCursor] at the latest local version of the associated
    /// sphere, mounted to that version. If the latest version changes due to
    /// effects in the distance, the cursor will still point to the same version
    /// it referred to when it was created.
    pub async fn mounted(has_sphere_context: C) -> Result<Self> {
        // let sphere_version = has_sphere_context.sphere_context().await?.head().await?;
        let mut cursor = Self::latest(has_sphere_context);
        cursor.mount().await?;
        Ok(cursor)
    }

    /// Create this [SphereCursor] at the latest local version of the associated
    /// sphere. The [SphereCursor] will always point to the latest local
    /// version, unless subsequently mounted.
    pub fn latest(has_sphere_context: C) -> Self {
        SphereCursor {
            has_sphere_context,
            key: PhantomData,
            storage: PhantomData,
            sphere_version: None,
        }
    }

    /// Rewind the [SphereCursor] to point to the version of the sphere just
    /// prior to this one in the edit chronology. If there was a previous
    /// version to rewind to then the returned `Option` has the [Cid] of the
    /// revision, otherwise if the current version is the oldest one it is
    /// `None`.
    pub async fn rewind(&mut self) -> Result<Option<Cid>> {
        let sphere = self.to_sphere().await?;

        match sphere.get_parent().await? {
            Some(parent) => {
                self.sphere_version = Some(*parent.cid());
                Ok(self.sphere_version)
            }
            None => Ok(None),
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<C, K, S> HasMutableSphereContext<K, S> for SphereCursor<C, K, S>
where
    C: HasMutableSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage,
{
    type MutableSphereContext = C::MutableSphereContext;

    async fn sphere_context_mut(&mut self) -> Result<Self::MutableSphereContext> {
        self.has_sphere_context.sphere_context_mut().await
    }

    async fn save(&mut self, additional_headers: Option<Vec<(String, String)>>) -> Result<Cid> {
        let new_version = self.has_sphere_context.save(additional_headers).await?;

        if self.sphere_version.is_some() {
            self.sphere_version = Some(new_version);
        }

        Ok(new_version)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<C, K, S> HasSphereContext<K, S> for SphereCursor<C, K, S>
where
    C: HasSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    type SphereContext = C::SphereContext;

    async fn sphere_context(&self) -> Result<Self::SphereContext> {
        self.has_sphere_context.sphere_context().await
    }

    async fn version(&self) -> Result<Cid> {
        match &self.sphere_version {
            Some(sphere_version) => Ok(*sphere_version),
            None => self.has_sphere_context.version().await,
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<'a, C, K, S> HasSphereContext<K, S> for &'a SphereCursor<C, K, S>
where
    C: HasSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    type SphereContext = C::SphereContext;

    async fn sphere_context(&self) -> Result<Self::SphereContext> {
        self.has_sphere_context.sphere_context().await
    }

    async fn version(&self) -> Result<Cid> {
        match &self.sphere_version {
            Some(sphere_version) => Ok(*sphere_version),
            None => self.has_sphere_context.version().await,
        }
    }
}

#[cfg(test)]
pub mod tests {

    use noosphere_core::data::{ContentType, Header};
    use tokio::io::AsyncReadExt;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use crate::helpers::{simulated_sphere_context, SimulationAccess};
    use crate::{HasMutableSphereContext, SphereContentRead, SphereContentWrite};

    use super::SphereCursor;

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_unlink_slugs_from_the_content_space() {
        let sphere_context = simulated_sphere_context(SimulationAccess::ReadWrite, None)
            .await
            .unwrap();
        let mut cursor = SphereCursor::latest(sphere_context);

        cursor
            .write(
                "cats",
                &ContentType::Subtext.to_string(),
                b"Cats are great".as_ref(),
                None,
            )
            .await
            .unwrap();

        cursor.save(None).await.unwrap();

        assert!(cursor.read("cats").await.unwrap().is_some());

        cursor.remove("cats").await.unwrap();
        cursor.save(None).await.unwrap();

        assert!(cursor.read("cats").await.unwrap().is_none());
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_flushes_on_every_save() {
        let sphere_context = simulated_sphere_context(SimulationAccess::ReadWrite, None)
            .await
            .unwrap();
        let initial_stats = {
            sphere_context
                .lock()
                .await
                .db()
                .to_block_store()
                .to_stats()
                .await
        };
        let mut cursor = SphereCursor::latest(sphere_context.clone());

        cursor
            .write(
                "cats",
                &ContentType::Subtext.to_string(),
                b"Cats are great".as_ref(),
                None,
            )
            .await
            .unwrap();

        cursor.save(None).await.unwrap();

        let first_save_stats = {
            sphere_context
                .lock()
                .await
                .db()
                .to_block_store()
                .to_stats()
                .await
        };

        assert_eq!(first_save_stats.flushes, initial_stats.flushes + 1);

        cursor.remove("cats").await.unwrap();
        cursor.save(None).await.unwrap();

        let second_save_stats = {
            sphere_context
                .lock()
                .await
                .db()
                .to_block_store()
                .to_stats()
                .await
        };

        assert_eq!(second_save_stats.flushes, first_save_stats.flushes + 1);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_does_not_allow_writes_when_an_author_has_read_only_access() {
        let sphere_context = simulated_sphere_context(SimulationAccess::Readonly, None)
            .await
            .unwrap();

        let mut cursor = SphereCursor::latest(sphere_context);

        let write_result = cursor
            .write(
                "cats",
                &ContentType::Subtext.to_string(),
                b"Cats are great".as_ref(),
                None,
            )
            .await;

        assert!(write_result.is_err());
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_write_a_file_and_read_it_back() {
        let sphere_context = simulated_sphere_context(SimulationAccess::ReadWrite, None)
            .await
            .unwrap();
        let mut cursor = SphereCursor::latest(sphere_context);

        cursor
            .write(
                "cats",
                &ContentType::Subtext.to_string(),
                b"Cats are great".as_ref(),
                None,
            )
            .await
            .unwrap();

        cursor.save(None).await.unwrap();

        let mut file = cursor.read("cats").await.unwrap().unwrap();

        file.memo
            .expect_header(
                &Header::ContentType.to_string(),
                &ContentType::Subtext.to_string(),
            )
            .unwrap();

        let mut value = String::new();
        file.contents.read_to_string(&mut value).await.unwrap();

        assert_eq!("Cats are great", value.as_str());
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_overwrite_a_file_with_new_contents_and_preserve_history() {
        let sphere_context = simulated_sphere_context(SimulationAccess::ReadWrite, None)
            .await
            .unwrap();
        let mut cursor = SphereCursor::latest(sphere_context);

        cursor
            .write(
                "cats",
                &ContentType::Subtext.to_string(),
                b"Cats are great".as_ref(),
                None,
            )
            .await
            .unwrap();

        cursor.save(None).await.unwrap();

        cursor
            .write(
                "cats",
                &ContentType::Subtext.to_string(),
                b"Cats are better than dogs".as_ref(),
                None,
            )
            .await
            .unwrap();

        cursor.save(None).await.unwrap();

        let mut file = cursor.read("cats").await.unwrap().unwrap();

        file.memo
            .expect_header(
                &Header::ContentType.to_string(),
                &ContentType::Subtext.to_string(),
            )
            .unwrap();

        let mut value = String::new();
        file.contents.read_to_string(&mut value).await.unwrap();

        assert_eq!("Cats are better than dogs", value.as_str());

        assert!(cursor.rewind().await.unwrap().is_some());

        file = cursor.read("cats").await.unwrap().unwrap();

        file.memo
            .expect_header(
                &Header::ContentType.to_string(),
                &ContentType::Subtext.to_string(),
            )
            .unwrap();

        value.clear();
        file.contents.read_to_string(&mut value).await.unwrap();

        assert_eq!("Cats are great", value.as_str());
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_throws_an_error_when_saving_without_changes() {
        let sphere_context = simulated_sphere_context(SimulationAccess::ReadWrite, None)
            .await
            .unwrap();
        let mut cursor = SphereCursor::latest(sphere_context);

        let result = cursor.save(None).await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "No changes to save");
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_throws_an_error_when_saving_with_empty_mutation_and_empty_headers() {
        let sphere_context = simulated_sphere_context(SimulationAccess::ReadWrite, None)
            .await
            .unwrap();
        let mut cursor = SphereCursor::latest(sphere_context);

        let result = cursor.save(Some(vec![])).await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "No changes to save");
    }
}
