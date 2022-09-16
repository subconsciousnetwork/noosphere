use anyhow::Result;
use horrorshow::html;
use noosphere_fs::SphereFs;
use noosphere_storage::interface::Store;

use crate::transclude::{Transclude, Transcluder};

/// Transforms a transclude into HTML
pub struct TranscludeToHtmlTransformer<'a, S>
where
    S: Store,
{
    transcluder: Transcluder<'a, S>,
}

impl<'a, S> TranscludeToHtmlTransformer<'a, S>
where
    S: Store,
{
    pub fn new(fs: &'a SphereFs<S>) -> Self {
        TranscludeToHtmlTransformer {
            transcluder: Transcluder::new(fs),
        }
    }

    pub async fn transform(&'a self, slug: &str) -> Result<Option<String>> {
        let transclude = self.transcluder.transclude(slug).await?;

        Ok(match transclude {
            Some(Transclude::Text(text_transclude)) => Some(
                html! {
                    li(class="transclude-item") {
                        a(class="transclude-format-text", href=&text_transclude.href) {
                            @ if let Some(title) = &text_transclude.title {
                                span(class="title") : title
                            }

                            @ if let Some(excerpt) = &text_transclude.excerpt {
                                span(class="excerpt") : excerpt
                            }

                            span(class="link-text") : &text_transclude.link_text
                        }
                    }
                }
                .to_string(),
            ),
            None => None,
        })
    }
}
