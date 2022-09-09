use std::str::FromStr;

use anyhow::Result;
use async_stream::try_stream;
use bytes::Bytes;
use futures::Stream;
use horrorshow::html;
use noosphere_storage::interface::Store;
use subtext::{block::Block, primitive::Entity};
use tokio::io::AsyncRead;
use tokio_stream::StreamExt;
use tokio_util::io::StreamReader;

use crate::{resolve::Resolver, slashlink::Slashlink, transclude::Transcluder};

use super::template;

pub struct SubtextToHtmlTransform<'a, S, R, T>
where
    S: Store,
    R: Resolver,
    T: Transcluder,
{
    store: &'a S,
    resolver: &'a R,
    transcluder: &'a T,
}

impl<'a, S, R, T> SubtextToHtmlTransform<'a, S, R, T>
where
    S: Store,
    R: Resolver,
    T: Transcluder,
{
    pub fn new(store: &'a S, resolver: &'a R, transcluder: &'a T) -> Self {
        SubtextToHtmlTransform {
            store,
            resolver,
            transcluder,
        }
    }

    pub async fn transform<B>(&'a self, byte_stream: B) -> impl AsyncRead + 'a
    where
        B: AsyncRead + Unpin + 'a,
    {
        StreamReader::new(self.html_text_stream(byte_stream).await)
    }

    async fn html_text_stream<B>(
        &'a self,
        byte_stream: B,
    ) -> impl Stream<Item = Result<Bytes, std::io::Error>> + 'a
    where
        B: AsyncRead + Unpin + 'a,
    {
        let subtext_ast_stream = subtext::stream::<Block<Entity>, Entity, _>(byte_stream).await;

        try_stream! {
            yield Bytes::from(template::document_prefix());

            for await block in subtext_ast_stream {
                if let Ok(block) = block {
                    yield Bytes::from(self.transform_block(block).await.map_err(|error| {
                        std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string())
                    })?);
                }
            }

            yield Bytes::from(template::document_suffix());
        }
    }

    async fn transform_block(&self, block: Block<Entity>) -> Result<String> {
        match block {
            Block::Header(_e) => Ok("Header".into()),
            Block::Paragraph(_) => Ok("Paragraph".into()),
            Block::Quote(_) => Ok("Block Quote".into()),
            Block::List(_) => Ok("List".into()),
            Block::Link(_) => Ok("Link".into()),
            Block::Seperator(_) => Ok("Sep".into()),
        }
    }

    async fn transform_entity(&self, entity: Entity) -> Result<String> {
        Ok(match entity {
            Entity::Sigil(_) => String::new(),
            Entity::TextSpan(content) => format!(
                "{}",
                html! {
                    p {
                        : content.to_string()
                    }
                }
            ),
            Entity::EmptySpace(_) => String::new(),
            Entity::SlashLink(text) => {
                let slash_link = Slashlink::from_str(text.as_ref())?;

                "".into()
            }
            Entity::HyperLink(_) => todo!(),
            Entity::WikiLink(_) => todo!(),
        })
    }
}
