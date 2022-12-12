use crate::ResolvedLink;
use anyhow::Result;
use async_trait::async_trait;
use subtext::Slashlink;

#[cfg(not(target_arch = "wasm32"))]
pub trait ResolverConditionalSendSync: Send + Sync {}

#[cfg(not(target_arch = "wasm32"))]
impl<S> ResolverConditionalSendSync for S where S: Send + Sync {}

#[cfg(target_arch = "wasm32")]
pub trait ResolverConditionalSendSync {}

#[cfg(target_arch = "wasm32")]
impl<S> ResolverConditionalSendSync for S {}

/// A [Resolver] is given a [Slashlink] and resolves it to a [ResolvedLink]
/// which includes an href, which is a URL string that can be used to link
/// to the content referred to by the [Slashlink] over the hypertext web.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait Resolver: Clone + ResolverConditionalSendSync {
    async fn resolve(&self, link: &Slashlink) -> Result<ResolvedLink>;
}
