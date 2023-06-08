use anyhow::{anyhow, Result};
use cid::Cid;
use libipld_cbor::DagCborCodec;
use noosphere_core::data::{BodyChunkIpld, Header, Link, MemoIpld};
use noosphere_storage::{BlockStore, Storage};

use tokio::io::AsyncReadExt;
use ucan::crypto::KeyMaterial;

use crate::{internal::SphereContextInternal, HasMutableSphereContext, HasSphereContext};
use async_trait::async_trait;

use crate::{AsyncFileBody, SphereContentRead};

fn validate_slug(slug: &str) -> Result<()> {
    if slug.is_empty() {
        Err(anyhow!("Slug must not be empty."))
    } else {
        Ok(())
    }
}

/// Anything that can write content to a sphere should implement
/// [SphereContentWrite]. A blanket implementation is provided for anything that
/// implements [HasMutableSphereContext].
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait SphereContentWrite<K, S>: SphereContentRead<K, S>
where
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    /// Like link, this takes a [Link<MemoIpld>] that should be associated directly with
    /// a slug, but in this case the [Link<MemoIpld>] is assumed to refer to a memo, so
    /// no wrapping memo is created.
    async fn link_raw(&mut self, slug: &str, cid: &Link<MemoIpld>) -> Result<()>;

    /// Similar to write, but instead of generating blocks from some provided
    /// bytes, the caller provides a CID of an existing DAG in storage. That
    /// CID is used as the body of a Memo that is written to the specified
    /// slug, and the CID of the memo is returned.
    async fn link(
        &mut self,
        slug: &str,
        content_type: &str,
        body_cid: &Cid,
        additional_headers: Option<Vec<(String, String)>>,
    ) -> Result<Link<MemoIpld>>;

    /// Write to a slug in the sphere. In order to commit the change to the
    /// sphere, you must call save. You can buffer multiple writes before
    /// saving.
    ///
    /// The returned CID is a link to the memo for the newly added content.
    async fn write<F: AsyncFileBody>(
        &mut self,
        slug: &str,
        content_type: &str,
        mut value: F,
        additional_headers: Option<Vec<(String, String)>>,
    ) -> Result<Link<MemoIpld>>;

    /// Unlinks a slug from the content space. Note that this does not remove
    /// the blocks that were previously associated with the content found at the
    /// given slug, because they will still be available at an earlier revision
    /// of the sphere. In order to commit the change, you must save. Note that
    /// this call is a no-op if there is no matching slug linked in the sphere.
    ///
    /// The returned value is the CID previously associated with the slug, if
    /// any.
    async fn remove(&mut self, slug: &str) -> Result<Option<Link<MemoIpld>>>;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<C, K, S> SphereContentWrite<K, S> for C
where
    C: HasSphereContext<K, S> + HasMutableSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    async fn link_raw(&mut self, slug: &str, cid: &Link<MemoIpld>) -> Result<()> {
        self.assert_write_access().await?;
        validate_slug(slug)?;

        self.sphere_context_mut()
            .await?
            .mutation_mut()
            .content_mut()
            .set(&slug.into(), cid);

        Ok(())
    }

    async fn link(
        &mut self,
        slug: &str,
        content_type: &str,
        body_cid: &Cid,
        additional_headers: Option<Vec<(String, String)>>,
    ) -> Result<Link<MemoIpld>> {
        self.assert_write_access().await?;
        validate_slug(slug)?;

        let memo_cid = {
            let current_file = self.read(slug).await?;
            let previous_memo_cid = current_file.map(|file| file.memo_version);

            let mut sphere_context = self.sphere_context_mut().await?;

            let mut new_memo = match previous_memo_cid {
                Some(cid) => {
                    let mut memo = MemoIpld::branch_from(&cid, sphere_context.db()).await?;
                    memo.body = *body_cid;
                    memo
                }
                None => MemoIpld {
                    parent: None,
                    headers: Vec::new(),
                    body: *body_cid,
                },
            };

            if let Some(headers) = additional_headers {
                new_memo.replace_headers(headers)
            }

            new_memo.replace_first_header(&Header::ContentType, content_type);

            // TODO(#43): Configure default/implicit headers here
            sphere_context
                .db_mut()
                .save::<DagCborCodec, MemoIpld>(new_memo)
                .await?
                .into()
        };

        self.link_raw(slug, &memo_cid).await?;

        Ok(memo_cid)
    }

    async fn write<F: AsyncFileBody>(
        &mut self,
        slug: &str,
        content_type: &str,
        mut value: F,
        additional_headers: Option<Vec<(String, String)>>,
    ) -> Result<Link<MemoIpld>> {
        debug!("Writing {}...", slug);

        self.assert_write_access().await?;
        validate_slug(slug)?;

        let mut bytes = Vec::new();
        value.read_to_end(&mut bytes).await?;

        // TODO(#38): We imply here that the only content types we care about
        // amount to byte streams, but in point of fact we can support anything
        // that may be referenced by CID including arbitrary IPLD structures
        let body_cid =
            BodyChunkIpld::store_bytes(&bytes, self.sphere_context_mut().await?.db_mut()).await?;

        self.link(slug, content_type, &body_cid, additional_headers)
            .await
    }

    async fn remove(&mut self, slug: &str) -> Result<Option<Link<MemoIpld>>> {
        self.assert_write_access().await?;

        let current_file = self.read(slug).await?;

        Ok(match current_file {
            Some(file) => {
                self.sphere_context_mut()
                    .await?
                    .mutation_mut()
                    .content_mut()
                    .remove(&String::from(slug));

                Some(file.memo_version)
            }
            None => None,
        })
    }
}
