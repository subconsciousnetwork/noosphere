use std::io::Cursor;

use anyhow::Result;
use async_stream::try_stream;
use bytes::Bytes;
use cid::Cid;
use futures::Stream;
use noosphere::{
    data::MemoIpld,
    view::{Links, Sphere, Timeline},
};
use noosphere_fs::SphereFs;
use noosphere_storage::interface::Store;
use tokio::io::AsyncRead;
use tokio_stream::StreamExt;
use tokio_util::io::StreamReader;

use crate::html::template::html_document_envelope;

use super::TranscludeToHtmlTransformer;

/// Transforms a sphere into HTML
pub struct SphereToHtmlTransformer<'a, S>
where
    S: Store,
{
    fs: &'a SphereFs<S>,
    transclude_transformer: TranscludeToHtmlTransformer<'a, S>,
}

impl<'a, S> SphereToHtmlTransformer<'a, S>
where
    S: Store,
{
    pub fn new(fs: &'a SphereFs<S>) -> Self {
        SphereToHtmlTransformer {
            fs,
            transclude_transformer: TranscludeToHtmlTransformer::new(fs),
        }
    }

    // TODO: Need a mechanism to enqueue additional transformations e.g., for linked media
    pub async fn transform(&'a self) -> Result<Option<impl AsyncRead + 'a>> {
        let sphere = self.fs.to_sphere();
        let memo = sphere.try_as_memo().await?;
        let links = sphere.try_get_links().await?;

        Ok(Some(Cursor::new(self.html_text(memo, links).await?)))
    }

    async fn html_text(&'a self, memo: MemoIpld, links: Links<S>) -> Result<String> {
        // TODO: Cheating here because the link stream isn't Send + Sync; should fix this
        let mut link_stream = links.stream().await?;
        let (html_prefix, html_suffix) = html_document_envelope(memo);
        let mut transcludes = Vec::new();

        while let Some(Ok((slug, _))) = link_stream.next().await {
            if let Some(transclude_html) = self.transclude_transformer.transform(slug).await? {
                transcludes.push(transclude_html)
            }
        }

        Ok(format!(
            r#"{}
{}
{}"#,
            html_prefix,
            transcludes.join("\n"),
            html_suffix
        ))
    }

    async fn html_text_stream(
        &'a self,
        memo: MemoIpld,
        links: Links<S>,
    ) -> impl Stream<Item = Result<Bytes, std::io::Error>> + 'a {
        try_stream! {
            // TODO: Discover a way to avoid all the mapping to std::io::Error
            let link_stream = links.stream().await.map_err(|error| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string())
            })?;

            let (html_prefix, html_suffix) = html_document_envelope(memo);

            yield Bytes::from(html_prefix);

            for await result in link_stream {
                let (slug, _) = result.map_err(|error| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string())
                })?;
                if let Some(transclude_html) = self.transclude_transformer.transform(&slug).await.map_err(|error| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string())
                })? {
                    yield Bytes::from(transclude_html);
                }
            }

            yield Bytes::from(html_suffix);
        }
    }
}
