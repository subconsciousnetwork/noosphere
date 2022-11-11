use std::str::FromStr;

use anyhow::Result;
use async_stream::try_stream;
use bytes::Bytes;
use futures::Stream;
use horrorshow::{html, Raw};
use noosphere_fs::{SphereFile, SphereFs};
use noosphere_storage::interface::Store;
use subtext::{block::Block, primitive::Entity, Slashlink};
use tokio::io::AsyncRead;
use tokio_util::io::StreamReader;
use ucan::crypto::KeyMaterial;

use crate::html::template::html_document_envelope;

use super::transclude::TranscludeToHtmlTransformer;

/// Transforms Subtext files from a sphere into HTML
pub struct SubtextToHtmlTransformer<'a, S, K>
where
    S: Store,
    K: KeyMaterial + Clone + 'static,
{
    fs: &'a SphereFs<S, K>,
    transclude_transformer: TranscludeToHtmlTransformer<'a, S, K>,
}

impl<'a, S, K> SubtextToHtmlTransformer<'a, S, K>
where
    S: Store,
    K: KeyMaterial + Clone + 'static,
{
    pub fn new(fs: &'a SphereFs<S, K>) -> Self {
        SubtextToHtmlTransformer {
            fs,
            transclude_transformer: TranscludeToHtmlTransformer::new(fs),
        }
    }

    // TODO(#55): Need a mechanism to enqueue additional transformations
    // e.g., for linked media
    pub async fn transform(&'a self, slug: &str) -> Result<Option<impl AsyncRead + 'a>> {
        let sphere_file = match self.fs.read(slug).await? {
            Some(sphere_file) => sphere_file,
            None => return Ok(None),
        };

        Ok(Some(StreamReader::new(
            self.html_text_stream(sphere_file).await,
        )))
    }

    async fn html_text_stream<B>(
        &'a self,
        sphere_file: SphereFile<B>,
    ) -> impl Stream<Item = Result<Bytes, std::io::Error>> + 'a
    where
        B: AsyncRead + Unpin + 'a,
    {
        let subtext_ast_stream =
            subtext::stream::<Block<Entity>, Entity, _>(sphere_file.contents).await;
        let (html_prefix, html_suffix) = html_document_envelope(sphere_file.memo);

        try_stream! {
            yield Bytes::from(html_prefix);

            for await block in subtext_ast_stream {
                if let Ok(block) = block {
                    yield Bytes::from(self.transform_block(block).await.map_err(|error| {
                        std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string())
                    })?);
                    yield Bytes::from("\n");
                }
            }

            yield Bytes::from(html_suffix);
        }
    }

    async fn transform_block(&self, block: Block<Entity>) -> Result<String> {
        let mut content_html_strings = Vec::new();
        let mut transclude_html_strings = Vec::new();
        let content_entities: Vec<Entity> =
            block.to_content_entities().into_iter().cloned().collect();
        let is_solo_slashlink = if let (Some(&Entity::SlashLink(_)), 1) =
            (content_entities.first(), content_entities.len())
        {
            true
        } else {
            false
        };

        for entity in content_entities {
            let (entity_html, transclude_html) = self.transform_entity(entity).await?;

            content_html_strings.push(entity_html);
            if let Some(transclude_html) = transclude_html {
                transclude_html_strings.push(transclude_html);
            }
        }

        let content_html = content_html_strings.join("\n");
        let content_html = match block {
            Block::Header(_) => html! {
                h1(class="block-header") : Raw(&content_html)
            }
            .to_string(),
            Block::Paragraph(_) => {
                // If this is a slashlink on its own, effectively replace it with its transclude
                if is_solo_slashlink {
                    String::new()
                } else {
                    html! {
                        p(class="block-paragraph") : Raw(&content_html)
                    }
                    .to_string()
                }
            }
            Block::Quote(_) => html! {
                blockquote(class="block-quote") : Raw(&content_html)
            }
            .to_string(),
            Block::List(_) => html! {
                div(class="block-list") : Raw(&content_html)
            }
            .to_string(),
            Block::Blank(_) => html! { p(class="block-blank") }.to_string(),
        };

        let transclude_html = transclude_html_strings.join("\n");

        Ok(html! {
            @if !content_html.is_empty() || !transclude_html.is_empty() {
                li(class="block") {
                    @if !content_html.is_empty() {
                        section(class="block-content") : Raw(&content_html);
                    }
                    @if !transclude_html.is_empty() {
                        ul(class="block-transcludes") : Raw(&transclude_html);
                    }
                }
            }
        }
        .to_string())
    }

    async fn transform_entity(&self, entity: Entity) -> Result<(String, Option<String>)> {
        Ok(match entity {
            Entity::Sigil(_) => (String::new(), None),
            Entity::TextSpan(content) => (
                format!(
                    "{}",
                    html! {
                        span(class="text") {
                            : content.to_string()
                        }
                    }
                ),
                None,
            ),
            Entity::EmptySpace(_) => (String::new(), None),
            Entity::SlashLink(text) => {
                let slashlink = Slashlink::from_str(text.as_ref())?;
                let slug = match &slashlink.slug {
                    Some(slug) => slug,
                    None => todo!("Support mention-style slashlinks"),
                }
                .clone();
                let slashlink = slashlink.to_string();
                let transclude = self.transclude_transformer.transform(&slug).await?;

                (
                    format!(
                        "{}",
                        html! {
                            a(href=format!("/{}", slug), class="slashlink") {
                                : &slashlink
                            }
                        }
                    ),
                    transclude,
                )
            }
            Entity::HyperLink(text) => (
                format!(
                    "{}",
                    html! {
                        a(href=format!("{}", text), class="hyperlink", target="_blank", rel="noopener")
                    }
                ),
                // TODO(#60): Discriminate by origin, if/when there is a known target publishing origin
                // TODO(#61): Support hyperlink transcludes
                None,
            ),
            Entity::WikiLink(text) => {
                let slug = subtext::util::to_slug(text.as_ref())?;
                // TODO(subconsciousnetwork/subconscious#328): For now, we are
                // not transcluding wikilinks; decide if we should eventually
                //let transclude = self.make_transclude(&slug).await?;

                (
                    format!(
                        "{}",
                        html! {
                            a(href=format!("/{}", slug), class="wikilink") {
                                : text.to_string()
                            }
                        }
                    ),
                    None,
                )
            }
        })
    }
}
