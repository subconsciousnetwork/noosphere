use anyhow::{anyhow, Result};
use async_trait::async_trait;
use cid::Cid;
use noosphere_core::{data::Did, view::Sphere};
use noosphere_storage::Storage;
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};
use tokio::sync::{Mutex, OwnedMutexGuard};
use ucan::crypto::KeyMaterial;

use super::SphereContext;

#[cfg(not(target_arch = "wasm32"))]
pub trait HasConditionalSendSync: Send + Sync {}

#[cfg(not(target_arch = "wasm32"))]
impl<S> HasConditionalSendSync for S where S: Send + Sync {}

#[cfg(target_arch = "wasm32")]
pub trait HasConditionalSendSync {}

#[cfg(target_arch = "wasm32")]
impl<S> HasConditionalSendSync for S {}

/// Any container that can provide non-mutable access to a [SphereContext]
/// should implement [HasSphereContext]. The most common example of something
/// that may implement this trait is an `Arc<SphereContext<_, _>>`. Implementors
/// of this trait will automatically implement other traits that provide
/// convience methods for accessing different parts of the sphere, such as
/// content and petnames.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait HasSphereContext<K, S>: Clone + HasConditionalSendSync
where
    K: KeyMaterial + Clone + 'static,
    S: Storage,
{
    type SphereContext: Deref<Target = SphereContext<K, S>> + HasConditionalSendSync;

    /// Get the [SphereContext] that is made available by this container.
    async fn sphere_context(&self) -> Result<Self::SphereContext>;

    /// Get the DID identity of the sphere that this FS view is reading from and
    /// writing to
    async fn identity(&self) -> Result<Did> {
        let sphere_context = self.sphere_context().await?;

        Ok(sphere_context.identity().clone())
    }

    /// The CID revision of the sphere that this FS view is reading from and
    /// writing to
    async fn version(&self) -> Result<Cid> {
        let identity = self.identity().await?;
        let sphere_context = self.sphere_context().await?;

        sphere_context
            .db()
            .get_version(&identity)
            .await?
            .ok_or_else(|| anyhow!("No version found for {}", identity))
    }

    /// Get a data view into the sphere at the current revision
    async fn to_sphere(&self) -> Result<Sphere<S::BlockStore>> {
        Ok(Sphere::at(
            &self.version().await?,
            &self.sphere_context().await?.db().to_block_store(),
        ))
    }
}

/// Any container that can provide mutable access to a [SphereContext] should
/// implement [HasMutableSphereContext]. The most common example of something
/// that may implement this trait is `Arc<Mutex<SphereContext<_, _>>>`.
/// Implementors of this trait will automatically implement other traits that
/// provide convenience methods for modifying the contents, petnames and other
/// aspects of a sphere.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait HasMutableSphereContext<K, S>: HasSphereContext<K, S> + HasConditionalSendSync
where
    K: KeyMaterial + Clone + 'static,
    S: Storage,
{
    type MutableSphereContext: Deref<Target = SphereContext<K, S>>
        + DerefMut<Target = SphereContext<K, S>>
        + HasConditionalSendSync;

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
    /// new version [Cid] of the sphere is returned. This method must be invoked
    /// in order to update the local history of the sphere with any changes that
    /// have been made.
    async fn save(&mut self, additional_headers: Option<Vec<(String, String)>>) -> Result<Cid> {
        let sphere = self.to_sphere().await?;
        let mut sphere_context = self.sphere_context_mut().await?;
        let sphere_identity = sphere_context.identity().clone();
        let mut revision = sphere.apply_mutation(sphere_context.mutation()).await?;

        match additional_headers {
            Some(headers) if !headers.is_empty() => revision.memo.replace_headers(headers),
            _ if sphere_context.mutation().is_empty() => return Err(anyhow!("No changes to save")),
            _ => (),
        }

        let new_sphere_version = revision
            .try_sign(
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

        Ok(new_sphere_version)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<K, S> HasSphereContext<K, S> for Arc<Mutex<SphereContext<K, S>>>
where
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    type SphereContext = OwnedMutexGuard<SphereContext<K, S>>;

    async fn sphere_context(&self) -> Result<Self::SphereContext> {
        Ok(self.clone().lock_owned().await)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<K, S> HasSphereContext<K, S> for Arc<SphereContext<K, S>>
where
    K: KeyMaterial + Clone + 'static,
    S: Storage,
{
    type SphereContext = Arc<SphereContext<K, S>>;

    async fn sphere_context(&self) -> Result<Self::SphereContext> {
        Ok(self.clone())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<K, S> HasMutableSphereContext<K, S> for Arc<Mutex<SphereContext<K, S>>>
where
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    type MutableSphereContext = OwnedMutexGuard<SphereContext<K, S>>;

    async fn sphere_context_mut(&mut self) -> Result<Self::MutableSphereContext> {
        self.sphere_context().await
    }
}
