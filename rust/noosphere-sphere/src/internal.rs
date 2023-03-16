use super::{BodyChunkDecoder, SphereFile};
use crate::{AsyncFileBody, HasSphereContext};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use libipld_cbor::DagCborCodec;
use noosphere_storage::{block_serialize, Storage};
use std::str::FromStr;
use tokio_util::io::StreamReader;
use ucan::crypto::KeyMaterial;

use cid::Cid;
use noosphere_core::{
    authority::Access,
    data::{ContentType, Header, MemoIpld},
};

/// A module-private trait for internal trait methods; this is a workaround for
/// the fact that all trait methods are implicitly public implementation
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub(crate) trait SphereContextInternal<K, S>
where
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    /// Returns an error result if the configured author of the [SphereContext]
    /// does not have write access to it (as a matter of cryptographic
    /// authorization).
    async fn assert_write_access(&self) -> Result<()>;

    async fn get_file(
        &self,
        sphere_revision: &Cid,
        memo: MemoIpld,
    ) -> Result<SphereFile<Box<dyn AsyncFileBody>>>;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<H, K, S> SphereContextInternal<K, S> for H
where
    H: HasSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    async fn assert_write_access(&self) -> Result<()> {
        let sphere_context = self.sphere_context().await?;
        match sphere_context.access().await? {
            Access::ReadOnly => Err(anyhow!(
                "Cannot mutate sphere; author only has read access to its contents"
            )),
            _ => Ok(()),
        }
    }

    async fn get_file(
        &self,
        sphere_revision: &Cid,
        memo: MemoIpld,
    ) -> Result<SphereFile<Box<dyn AsyncFileBody>>> {
        let sphere_context = self.sphere_context().await?;
        let (memo_version, _) = block_serialize::<DagCborCodec, _>(&memo)?;
        let content_type = match memo.get_first_header(&Header::ContentType.to_string()) {
            Some(content_type) => Some(ContentType::from_str(content_type.as_str())?),
            None => None,
        };

        let stream = match content_type {
            // TODO(#86): Content-type aware decoding of body bytes
            Some(_) => BodyChunkDecoder(&memo.body, sphere_context.db()).stream(),
            None => return Err(anyhow!("No content type specified")),
        };

        Ok(SphereFile {
            sphere_identity: sphere_context.identity().clone(),
            sphere_version: *sphere_revision,
            memo_version,
            memo,
            // NOTE: we have to box here because traits don't support `impl` types in return values
            contents: Box::new(StreamReader::new(stream)),
        })
    }
}
