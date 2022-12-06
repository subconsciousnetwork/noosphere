use std::{pin::Pin, str::FromStr};

use crate::transclude::{TextTransclude, Transclude};
use anyhow::Result;
use async_stream::stream;
use async_trait::async_trait;
use futures::Stream;
use horrorshow::{html, Raw};
use noosphere_core::data::{ContentType, Header};
use noosphere_fs::{SphereFile, SphereFs};
use noosphere_storage::Storage;
use subtext::{block::Block, primitive::Entity, Peer, Slashlink};
use tokio::{io::AsyncRead, sync::OnceCell};
use tokio_stream::StreamExt;
use ucan::crypto::KeyMaterial;
use url::Url;

pub enum Link {
    Hyperlink(Url),
    Slashlink(Slashlink),
}

#[cfg(not(target_arch = "wasm32"))]
pub trait ResolverConditionalSendSync: Send + Sync {}

#[cfg(not(target_arch = "wasm32"))]
impl<S> ResolverConditionalSendSync for S where S: Send + Sync {}

#[cfg(target_arch = "wasm32")]
pub trait ResolverConditionalSendSync {}

#[cfg(target_arch = "wasm32")]
impl<S> ResolverConditionalSendSync for S {}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait Resolver: Clone + ResolverConditionalSendSync {
    async fn resolve(&self, link: &Link) -> Result<Url>;
}

#[cfg(not(target_arch = "wasm32"))]
pub trait TranscluderConditionalSendSync: Send + Sync {}

#[cfg(not(target_arch = "wasm32"))]
impl<S> TranscluderConditionalSendSync for S where S: Send + Sync {}

#[cfg(target_arch = "wasm32")]
pub trait TranscluderConditionalSendSync {}

#[cfg(target_arch = "wasm32")]
impl<S> TranscluderConditionalSendSync for S {}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait Transcluder: Clone + TranscluderConditionalSendSync {
    async fn transclude(&self, link: &Link, href: &Url) -> Result<Option<Transclude>>;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait Transformer {
    type Resolver: Resolver;
    type Transcluder: Transcluder;

    fn resolver(&self) -> &Self::Resolver;
    fn transcluder(&self) -> &Self::Transcluder;

    fn transform_file<'a, R>(
        &'a self,
        file: SphereFile<R>,
    ) -> Pin<Box<dyn Stream<Item = String> + 'a>>
    where
        R: AsyncRead + Unpin + 'a;

    async fn transform_transclude(&self, transclude: Transclude) -> Result<String> {
        match transclude {
            Transclude::Text(text_transclude) => Ok(html! {
                aside(class="transclude") {
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
            .to_string()),
        }
    }
}

pub struct HtmlTransformer<R, T>
where
    R: Resolver,
    T: Transcluder,
{
    resolver: R,
    transcluder: T,

    subtext_transformer: OnceCell<SubtextToHtmlTransformer<R, T>>,
}
impl<R, T> HtmlTransformer<R, T>
where
    R: Resolver,
    T: Transcluder,
{
    pub fn new(resolver: R, transcluder: T) -> Self {
        HtmlTransformer {
            resolver,
            transcluder,
            subtext_transformer: OnceCell::new(),
        }
    }
}

impl<R, T> Transformer for HtmlTransformer<R, T>
where
    R: Resolver,
    T: Transcluder,
{
    type Resolver = R;
    type Transcluder = T;

    fn resolver(&self) -> &Self::Resolver {
        &self.resolver
    }

    fn transcluder(&self) -> &Self::Transcluder {
        &self.transcluder
    }

    fn transform_file<'a, Re>(
        &'a self,
        file: SphereFile<Re>,
    ) -> Pin<Box<dyn Stream<Item = String> + 'a>>
    where
        Re: AsyncRead + Unpin + 'a,
    {
        Box::pin(stream! {
          match file.memo.content_type() {
              Some(ContentType::Subtext) => {
                let subtext_transformer = self.subtext_transformer.get_or_init(|| async {
                  SubtextToHtmlTransformer::new(self.resolver.clone(), self.transcluder.clone())
                }).await;

                let stream = subtext_transformer.transform_file(file);
                for await part in stream {
                    yield part;
                }
              }
              _ => yield String::new(),
          };
        })
    }
}

#[derive(Clone)]
pub struct SubtextToHtmlTransformer<R, T>
where
    R: Resolver,
    T: Transcluder,
{
    resolver: R,
    transcluder: T,
}

impl<R, T> SubtextToHtmlTransformer<R, T>
where
    R: Resolver,
    T: Transcluder,
{
    pub fn new(resolver: R, transcluder: T) -> Self {
        SubtextToHtmlTransformer {
            resolver,
            transcluder,
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
            Block::Blank(_) => String::new(),
        };

        let transclude_html = transclude_html_strings.join("\n");

        Ok(html! {
            @if !content_html.is_empty() || !transclude_html.is_empty() {
                section(class="block") {
                    @if !content_html.is_empty() {
                        section(class="block-content") : Raw(&content_html);
                    }
                    @if !transclude_html.is_empty() {
                        section(class="block-transcludes") : Raw(&transclude_html);
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
                html! {
                    span(class="text") {
                        : content.to_string()
                    }
                }
                .to_string(),
                None,
            ),
            Entity::EmptySpace(_) => (String::new(), None),
            Entity::SlashLink(text) => {
                let slashlink = Slashlink::from_str(text.as_ref())?;
                let content = slashlink.to_string();
                let link = Link::Slashlink(slashlink);
                let href = self.resolver.resolve(&link).await?;

                let transclude = self.transcluder.transclude(&link, &href).await?;

                let transclude = if let Some(transclude) = transclude {
                    Some(self.transform_transclude(transclude).await?)
                } else {
                    None
                };

                (
                    html! {
                        a(href=href.to_string(), class="slashlink") {
                            : &content
                        }
                    }
                    .to_string(),
                    transclude,
                )
            }
            Entity::HyperLink(text) => {
                let link = Link::Hyperlink(Url::from_str(text.as_ref())?);
                let href = self.resolver.resolve(&link).await?;

                (
                    html! {
                        a(href=href.to_string(), class="hyperlink", target="_blank", rel="noopener") {
                            : href.to_string()
                        }
                    }
                    .to_string(),
                    // TODO(#60): Discriminate by origin, if/when there is a known target publishing origin
                    // TODO(#61): Support hyperlink transcludes
                    None,
                )
            }
            Entity::WikiLink(text) => {
                let slug = subtext::util::to_slug(text.as_ref())?;
                let slashlink = Slashlink::from_str(&format!("/{slug}"))?;
                let link = Link::Slashlink(slashlink);
                let href = self.resolver.resolve(&link).await?;

                let text = text
                    .strip_prefix("[[")
                    .unwrap_or_default()
                    .strip_suffix("]]")
                    .unwrap_or_default();

                // TODO(subconsciousnetwork/subconscious#328): For now, we are
                // not transcluding wikilinks; decide if we should eventually
                //let transclude = self.make_transclude(&slug).await?;

                (
                    html! {
                        a(href=href.to_string(), class="wikilink") {
                            span(class="wikilink-open-bracket") {
                                : "[["
                            }
                            span(class="wikilink-text") {
                                : text
                            }
                            span(class="wikilink-close-bracket") {
                                : "]]"
                            }
                        }
                    }
                    .to_string(),
                    None,
                )
            }
        })
    }
}

impl<R, T> Transformer for SubtextToHtmlTransformer<R, T>
where
    R: Resolver,
    T: Transcluder,
{
    type Resolver = R;

    type Transcluder = T;

    fn resolver(&self) -> &Self::Resolver {
        &self.resolver
    }

    fn transcluder(&self) -> &Self::Transcluder {
        &self.transcluder
    }

    fn transform_file<'a, Re>(
        &'a self,
        file: SphereFile<Re>,
    ) -> Pin<Box<dyn Stream<Item = String> + 'a>>
    where
        Re: AsyncRead + Unpin + 'a,
    {
        Box::pin(stream! {
            match file.memo.content_type() {
                Some(ContentType::Subtext) => (),
                actual => {
                    warn!("Input did not have the correct content-type; expected {}, got {:?}", ContentType::Subtext, actual.map(|content_type| content_type.to_string()).ok_or("nothing"));
                    yield String::new()
                }
            };

            let subtext_ast_stream = subtext::stream::<Block<Entity>, Entity, _>(file.contents).await;

            yield "<article class=\"subtext\">".into();

            for await block in subtext_ast_stream {
                if let Ok(block) = block {
                    match self.transform_block(block).await {
                      Ok(block_html) => {
                        yield block_html;
                        yield "\n".into();
                      },
                      Err(error) => {
                        warn!("Failed to transform subtext block: {:?}", error);
                      }
                    }
                }
            }

            yield "</article>".into();
        })
    }
}

/// A transcluder is responsible for taking a slug and generating a transclude
/// for the content that the slug refers to.
#[derive(Clone)]
pub struct SphereFsTranscluder<'a, S, K>
where
    S: Storage,
    K: KeyMaterial + Clone + 'static,
{
    fs: &'a SphereFs<S, K>,
}

impl<'a, S, K> SphereFsTranscluder<'a, S, K>
where
    S: Storage,
    K: KeyMaterial + Clone + 'static,
{
    pub fn new(fs: &'a SphereFs<S, K>) -> Self {
        SphereFsTranscluder { fs }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<'a, S, K> Transcluder for SphereFsTranscluder<'a, S, K>
where
    S: Storage,
    K: KeyMaterial + Clone + 'static,
{
    async fn transclude(&self, link: &Link, href: &Url) -> Result<Option<Transclude>> {
        match link {
            Link::Hyperlink(_) => {
                // TODO(#50): Support hyperlinks
                Ok(None)
            }
            Link::Slashlink(slashlink) => {
                match slashlink.peer {
                    Peer::None => {
                        // TODO(#49): Perhaps this should be sensitive to external content
                        // e.g., from other spheres
                        ()
                    }
                    _ => return Ok(None),
                };

                let slug = match &slashlink.slug {
                    Some(slug) => slug.to_owned(),
                    None => return Ok(None),
                };
                // TODO(#50): Support content types other than Subtext

                Ok(match self.fs.read(&slug).await? {
                    Some(file) => {
                        // TODO(#52): Maybe fall back to first heading if present and use
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
                            href: href.to_string(),
                        }))
                    }
                    None => {
                        // TODO(#53): Figure out how to treat "dead" links for HTML generation
                        // purposes; it may be that we want some dynamic widget that
                        // determines the liveness of a transclude at render time
                        Some(Transclude::Text(TextTransclude {
                            title: None,
                            excerpt: None,
                            link_text: format!("/{}", slug),
                            href: href.to_string(),
                        }))
                    }
                })
            }
        }
    }
}
