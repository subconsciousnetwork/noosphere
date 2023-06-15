use std::marker::PhantomData;

use crate::{ResolvedLink, TextTransclude, Transclude, Transcluder};
use anyhow::Result;
use async_trait::async_trait;
use noosphere_core::data::Header;
use noosphere_sphere::{HasSphereContext, SphereContentRead};
use noosphere_storage::Storage;
use subtext::{block::Block, primitive::Entity, Peer};
use tokio_stream::StreamExt;

/// A [Transcluder] implementation that uses [HasSphereContext] to resolve the content
/// being transcluded.
#[derive(Clone)]
pub struct SphereContentTranscluder<R, S>
where
    R: HasSphereContext<S> + Clone,
    S: Storage + 'static,
{
    content: R,
    storage_type: PhantomData<S>,
}

impl<R, S> SphereContentTranscluder<R, S>
where
    R: HasSphereContext<S> + Clone,
    S: Storage + 'static,
{
    pub fn new(content: R) -> Self {
        SphereContentTranscluder {
            content,
            storage_type: PhantomData,
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<R, S> Transcluder for SphereContentTranscluder<R, S>
where
    R: HasSphereContext<S> + Clone,
    S: Storage + 'static,
{
    async fn transclude(&self, link: &ResolvedLink) -> Result<Option<Transclude>> {
        match link {
            ResolvedLink::Hyperlink { .. } => {
                // TODO(#50): Support hyperlinks
                Ok(None)
            }
            ResolvedLink::Slashlink { link, href } => {
                match link.peer {
                    Peer::None => {
                        // TODO(#49): Perhaps this should be sensitive to external content
                        // e.g., from other spheres
                    }
                    _ => return Ok(None),
                };

                let slug = match &link.slug {
                    Some(slug) => slug.to_owned(),
                    None => return Ok(None),
                };
                // TODO(#50): Support content types other than Subtext

                Ok(match self.content.read(&slug).await? {
                    Some(file) => {
                        // TODO(#52): Maybe fall back to first heading if present and use
                        // that as a stand-in for title...
                        let title = file.memo.get_first_header(&Header::Title);

                        let subtext_ast_stream =
                            subtext::stream::<Block<Entity>, _, _>(file.contents).await;

                        tokio::pin!(subtext_ast_stream);

                        let mut excerpt = None;

                        while let Some(Ok(block)) = subtext_ast_stream.next().await {
                            match block {
                                Block::Blank(_) => continue,
                                any_other => {
                                    excerpt = Some(any_other.to_text_content());
                                    break;
                                }
                            }
                        }

                        Some(Transclude::Text(TextTransclude {
                            title,
                            excerpt,
                            link_text: format!("/{slug}"),
                            href: href.to_owned(),
                        }))
                    }
                    None => {
                        // TODO(#53): Figure out how to treat "dead" links for HTML generation
                        // purposes; it may be that we want some dynamic widget that
                        // determines the liveness of a transclude at render time
                        Some(Transclude::Text(TextTransclude {
                            title: None,
                            excerpt: None,
                            link_text: format!("/{slug}"),
                            href: href.to_owned(),
                        }))
                    }
                })
            }
        }
    }
}
