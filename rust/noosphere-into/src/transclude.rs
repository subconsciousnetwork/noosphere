use anyhow::Result;
use noosphere::data::Header;
use noosphere_fs::SphereFs;
use noosphere_storage::interface::Store;
use subtext::{block::Block, primitive::Entity};
use tokio_stream::StreamExt;

#[derive(Clone, Debug)]
pub struct TextTransclude {
    pub title: Option<String>,
    pub excerpt: Option<String>,
    pub link_text: String,
    pub href: String,
}

/// The set of possible transcludes that may need to be rendered to a target
/// format. At this time, only text transcludes are supported.
#[derive(Clone, Debug)]
pub enum Transclude {
    // TODO
    // Rich,
    // Interactive,
    // Bitmap,
    Text(TextTransclude),
}

/// A transcluder is responsible for taking a slug and generating a transclude
/// for the content that the slug refers to.
pub struct Transcluder<'a, S>
where
    S: Store,
{
    fs: &'a SphereFs<S>,
}

impl<'a, S> Transcluder<'a, S>
where
    S: Store,
{
    pub fn new(fs: &'a SphereFs<S>) -> Self {
        Transcluder { fs }
    }

    /// Generate a transclude for the given slug.
    pub async fn transclude(&self, slug: &str) -> Result<Option<Transclude>> {
        // TODO: Perhaps this should be sensitive to external content e.g., from other spheres
        // TODO: Support content types other than Subtext

        Ok(match self.fs.read(slug).await? {
            Some(file) => {
                // TODO: Maybe fall back to first heading if present and use
                // that as a stand-in for title...
                let title = file.memo.get_first_header(&Header::Title.to_string());

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
                    link_text: format!("/{}", slug),
                    href: format!("/{}", slug),
                }))
            }
            None => {
                // TODO: Figure out how to treat "dead" links for HTML generation
                // purposes; it may be that we want some dynamic widget that
                // determines the liveness of a transclude at render time
                Some(Transclude::Text(TextTransclude {
                    title: None,
                    excerpt: None,
                    link_text: format!("/{}", slug),
                    href: format!("/{}", slug),
                }))
            }
        })
    }
}
