use async_stream::stream;
use futures::Stream;
use noosphere_core::data::ContentType;
use noosphere_sphere::SphereFile;
use tokio::io::AsyncRead;

use crate::{subtext_to_html_document_stream, subtext_to_html_fragment_stream, Transform};

/// Used to configure the output format of the [file_to_html_stream] transform
pub enum HtmlOutput {
    /// Output as a full HTML document
    Document,
    /// Output as just a body content fragment
    Fragment,
}

/// Given a [Transform], a [SphereFile] and an [HtmlOutput], perform a streamed
/// transformation of the [SphereFile] into HTML. The transformation that is
/// performed may vary depending on content type. At this time, only Subtext
/// is supported.
pub fn file_to_html_stream<T, R>(
    file: SphereFile<R>,
    output: HtmlOutput,
    transform: T,
) -> impl Stream<Item = String>
where
    T: Transform,
    R: AsyncRead + Unpin,
{
    stream! {
        match file.memo.content_type() {
            Some(ContentType::Subtext) => {
                match output {
                    HtmlOutput::Document => {
                        let stream = subtext_to_html_document_stream(transform, file);
                        for await part in stream {
                            yield part;
                        }
                    },
                    HtmlOutput::Fragment => {
                        let stream = subtext_to_html_fragment_stream(file, transform);
                        for await part in stream {
                            yield part;
                        }
                    }
                };
            }
            _ => {
                yield "<article><p>Format cannot be rendered as HTML</p></article>".into();
            }
        };
    }
}
