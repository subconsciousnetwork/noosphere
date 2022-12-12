use crate::{ResolvedLink, Transclude};

use anyhow::Result;
use async_trait::async_trait;

#[cfg(not(target_arch = "wasm32"))]
pub trait TranscluderConditionalSendSync: Send + Sync {}

#[cfg(not(target_arch = "wasm32"))]
impl<S> TranscluderConditionalSendSync for S where S: Send + Sync {}

#[cfg(target_arch = "wasm32")]
pub trait TranscluderConditionalSendSync {}

#[cfg(target_arch = "wasm32")]
impl<S> TranscluderConditionalSendSync for S {}
/// A [Transcluder] is responsible for taking a slug and generating a transclude
/// for the content that the slug refers to.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait Transcluder: Clone + TranscluderConditionalSendSync {
    /// Given a [ResolvedLink], produce a [Transclude] if it is appropriate to
    /// do so.
    async fn transclude(&self, link: &ResolvedLink) -> Result<Option<Transclude>>;
}
