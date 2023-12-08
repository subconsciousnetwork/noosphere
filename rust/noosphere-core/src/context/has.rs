use crate::{
    authority::Author,
    data::{Did, Link, MemoIpld},
    view::Sphere,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use noosphere_common::ConditionalSync;
use noosphere_storage::{SphereDb, Storage};
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};
use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::context::SphereContextKey;

use super::SphereContext;

/// Any container that can provide non-mutable access to a [SphereContext]
/// should implement [HasSphereContext]. The most common example of something
/// that may implement this trait is an `Arc<SphereContext<_, _>>`. Implementors
/// of this trait will automatically implement other traits that provide
/// convience methods for accessing different parts of the sphere, such as
/// content and petnames.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait HasSphereContext<S>: Clone + ConditionalSync
where
    S: Storage + 'static,
{
    /// The type of the internal read-only [SphereContext]
    type SphereContext: Deref<Target = SphereContext<S>> + ConditionalSync;

    /// Get the [SphereContext] that is made available by this container.
    async fn sphere_context(&self) -> Result<Self::SphereContext>;

    /// Get the DID identity of the sphere that this FS view is reading from and
    /// writing to
    async fn identity(&self) -> Result<Did> {
        let sphere_context = self.sphere_context().await?;

        Ok(sphere_context.identity().clone())
    }

    /// The CID of the most recent local version of this sphere
    async fn version(&self) -> Result<Link<MemoIpld>> {
        self.sphere_context().await?.version().await
    }

    /// Get a data view into the sphere at the current revision
    async fn to_sphere(&self) -> Result<Sphere<SphereDb<S>>> {
        let version = self.version().await?;
        Ok(Sphere::at(&version, self.sphere_context().await?.db()))
    }

    /// Create a new [SphereContext] via [SphereContext::with_author] and wrap it in the same
    /// [HasSphereContext] implementation, returning the result
    async fn with_author(&self, author: &Author<SphereContextKey>) -> Result<Self> {
        Ok(Self::wrap(self.sphere_context().await?.with_author(author).await?).await)
    }

    /// Wrap a given [SphereContext] in this [HasSphereContext]
    async fn wrap(sphere_context: SphereContext<S>) -> Self;
}

/// Any container that can provide mutable access to a [SphereContext] should
/// implement [HasMutableSphereContext]. The most common example of something
/// that may implement this trait is `Arc<Mutex<SphereContext<_, _>>>`.
/// Implementors of this trait will automatically implement other traits that
/// provide convenience methods for modifying the contents, petnames and other
/// aspects of a sphere.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait HasMutableSphereContext<S>: HasSphereContext<S> + ConditionalSync
where
    S: Storage + 'static,
{
    /// The type of the internal mutable [SphereContext]
    type MutableSphereContext: Deref<Target = SphereContext<S>>
        + DerefMut<Target = SphereContext<S>>
        + ConditionalSync;

    /// Get a mutable reference to the [SphereContext] that is wrapped by this
    /// container.
    async fn sphere_context_mut(&mut self) -> Result<Self::MutableSphereContext>;

    /// Returns true if any changes have been made to the underlying
    /// [SphereContext] that have not been committed to the associated sphere
    /// yet (according to local history).
    async fn has_unsaved_changes(&self) -> Result<bool> {
        let context = self.sphere_context().await?;
        Ok(!context.mutation().is_empty())
    }

    /// Commits a series of writes to the sphere and signs the new version. The
    /// new version [Link<MemoIpld>] of the sphere is returned. This method must
    /// be invoked in order to update the local history of the sphere with any
    /// changes that have been made.
    #[instrument(level = "debug", skip(self))]
    async fn save(
        &mut self,
        additional_headers: Option<Vec<(String, String)>>,
    ) -> Result<Link<MemoIpld>> {
        let sphere = self.to_sphere().await?;
        let mut sphere_context = self.sphere_context_mut().await?;
        let sphere_identity = sphere_context.identity().clone();
        let mut revision = sphere.apply_mutation(sphere_context.mutation()).await?;

        debug!(?sphere_identity, "Saving sphere");

        match additional_headers {
            Some(headers) if !headers.is_empty() => revision.memo.replace_headers(headers),
            _ if sphere_context.mutation().is_empty() => return Err(anyhow!("No changes to save")),
            _ => (),
        }

        let new_sphere_version = revision
            .sign(
                &sphere_context.author().key,
                sphere_context.author().authorization.as_ref(),
            )
            .await?;

        sphere_context
            .db_mut()
            .set_version(&sphere_identity, &new_sphere_version)
            .await?;
        sphere_context.db_mut().flush().await?;
        sphere_context.mutation_mut().reset();

        debug!(?sphere_identity, ?new_sphere_version, "Sphere saved");

        Ok(new_sphere_version)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S> HasSphereContext<S> for Arc<Mutex<SphereContext<S>>>
where
    S: Storage + 'static,
{
    type SphereContext = OwnedMutexGuard<SphereContext<S>>;

    async fn sphere_context(&self) -> Result<Self::SphereContext> {
        Ok(self.clone().lock_owned().await)
    }

    async fn wrap(sphere_context: SphereContext<S>) -> Self {
        Arc::new(Mutex::new(sphere_context))
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S, T> HasSphereContext<S> for Box<T>
where
    T: HasSphereContext<S>,
    S: Storage + 'static,
{
    type SphereContext = T::SphereContext;

    async fn sphere_context(&self) -> Result<Self::SphereContext> {
        T::sphere_context(self).await
    }

    async fn wrap(sphere_context: SphereContext<S>) -> Self {
        Box::new(T::wrap(sphere_context).await)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S> HasSphereContext<S> for Arc<SphereContext<S>>
where
    S: Storage,
{
    type SphereContext = Arc<SphereContext<S>>;

    async fn sphere_context(&self) -> Result<Self::SphereContext> {
        Ok(self.clone())
    }

    async fn wrap(sphere_context: SphereContext<S>) -> Self {
        Arc::new(sphere_context)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S> HasMutableSphereContext<S> for Arc<Mutex<SphereContext<S>>>
where
    S: Storage + 'static,
{
    type MutableSphereContext = OwnedMutexGuard<SphereContext<S>>;

    async fn sphere_context_mut(&mut self) -> Result<Self::MutableSphereContext> {
        self.sphere_context().await
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S, T> HasMutableSphereContext<S> for Box<T>
where
    T: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    type MutableSphereContext = T::MutableSphereContext;

    async fn sphere_context_mut(&mut self) -> Result<Self::MutableSphereContext> {
        T::sphere_context_mut(self).await
    }
}
