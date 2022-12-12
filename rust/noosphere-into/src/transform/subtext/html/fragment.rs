use std::str::FromStr;

use crate::{Resolver, Transclude, Transcluder, Transform};
use anyhow::Result;
use async_stream::stream;

use futures::Stream;
use horrorshow::{html, Raw};
use noosphere_core::data::ContentType;
use noosphere_fs::SphereFile;

use subtext::{block::Block, primitive::Entity, Slashlink};
use tokio::io::AsyncRead;

use url::Url;

/// Given a [Transform] and a [SphereFile], produce a stream that yields the
/// file content as an HTML fragment
pub fn subtext_to_html_fragment_stream<T, R>(
    transform: T,
    file: SphereFile<R>,
) -> impl Stream<Item = String>
where
    T: Transform,
    R: AsyncRead + Unpin,
{
    stream! {
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
                match block_to_html(transform.clone(), block).await {
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
    }
}

/// Given a [Transform] and a [Block], produce an HTML string
pub async fn block_to_html<T>(transform: T, block: Block<Entity>) -> Result<String>
where
    T: Transform,
{
    let mut content_html_strings = Vec::new();
    let mut transclude_html_strings = Vec::new();
    let content_entities: Vec<Entity> = block.to_content_entities().into_iter().cloned().collect();
    let is_solo_slashlink = if let (Some(&Entity::SlashLink(_)), 1) =
        (content_entities.first(), content_entities.len())
    {
        true
    } else {
        false
    };

    for entity in content_entities {
        let (entity_html, transclude_html) = entity_to_html(transform.clone(), entity).await?;

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

/// Given a [Transform] and an [Entity], produce an HTML string
pub async fn entity_to_html<T>(transform: T, entity: Entity) -> Result<(String, Option<String>)>
where
    T: Transform,
{
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
            let link = transform.resolver().resolve(&slashlink).await?;

            let transclude = transform.transcluder().transclude(&link).await?;

            let transclude = if let Some(transclude) = transclude {
                Some(transclude_to_html(transclude).await?)
            } else {
                None
            };

            let href = link.to_string();

            (
                html! {
                    a(href=href.as_str(), class="slashlink") {
                        : &content
                    }
                }
                .to_string(),
                transclude,
            )
        }
        Entity::HyperLink(text) => {
            let href = Url::from_str(text.as_ref())?.to_string();

            (
                html! {
                    a(href=href.as_str(), class="hyperlink", target="_blank", rel="noopener") {
                        : href.as_str()
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
            let link = transform.resolver().resolve(&slashlink).await?;

            let text = text
                .strip_prefix("[[")
                .unwrap_or_default()
                .strip_suffix("]]")
                .unwrap_or_default();

            // TODO(subconsciousnetwork/subconscious#328): For now, we are
            // not transcluding wikilinks; decide if we should eventually
            //let transclude = self.make_transclude(&slug).await?;
            let href = link.to_string();

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

/// Convert a [Transclude] to an HTML string
pub async fn transclude_to_html(transclude: Transclude) -> Result<String> {
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
