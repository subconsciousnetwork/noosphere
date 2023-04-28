use anyhow::{anyhow, Result};
use async_trait::async_trait;
use subtext::{Peer, Slashlink};

use crate::{ResolvedLink, Resolver};

/// A [Resolver] that is suitable for resolving a [Slashlink] to an `href` for
/// a basic static website generator.
#[derive(Clone)]
pub struct StaticHtmlResolver();

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl Resolver for StaticHtmlResolver {
    async fn resolve(&self, link: &Slashlink) -> Result<ResolvedLink> {
        match link {
            Slashlink {
                slug: Some(slug),
                peer: Peer::None,
            } => Ok(ResolvedLink::Slashlink {
                link: link.clone(),
                href: format!("/{slug}"),
            }),
            _ => Err(anyhow!("Only local slashlinks with slugs are supported")),
        }
    }
}
