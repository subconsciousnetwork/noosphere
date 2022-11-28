use std::io::Cursor;

use anyhow::Result;
use noosphere_core::{data::MemoIpld, view::Links};
use noosphere_fs::SphereFs;
use noosphere_storage::Storage;
use tokio::io::AsyncRead;
use tokio_stream::StreamExt;
use ucan::crypto::KeyMaterial;

use crate::html::template::html_document_envelope;

use super::TranscludeToHtmlTransformer;

/// Transforms a sphere into HTML
pub struct SphereToHtmlTransformer<'a, S, K>
where
    S: Storage,
    K: KeyMaterial + Clone + 'static,
{
    fs: &'a SphereFs<S, K>,
    transclude_transformer: TranscludeToHtmlTransformer<'a, S, K>,
}

impl<'a, S, K> SphereToHtmlTransformer<'a, S, K>
where
    S: Storage,
    K: KeyMaterial + Clone + 'static,
{
    pub fn new(fs: &'a SphereFs<S, K>) -> Self {
        SphereToHtmlTransformer {
            fs,
            transclude_transformer: TranscludeToHtmlTransformer::new(fs),
        }
    }

    // TODO(#55): Need a mechanism to enqueue additional transformations
    // e.g., for linked media
    pub async fn transform(&'a self) -> Result<Option<impl AsyncRead + 'a>> {
        let sphere = self.fs.to_sphere();
        let memo = sphere.try_as_memo().await?;
        let links = sphere.try_get_links().await?;

        Ok(Some(Cursor::new(self.html_text(memo, links).await?)))
    }

    async fn html_text(&'a self, memo: MemoIpld, links: Links<S::BlockStore>) -> Result<String> {
        // Cheating here because the link stream isn't Send + Sync; should fix this
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
}
