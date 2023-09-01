use crate::{html_document_envelope, subtext_to_html_fragment_stream, Transform};
use async_stream::stream;
use futures::Stream;
use noosphere_core::context::SphereFile;
use tokio::io::AsyncRead;

/// Given a [Transform] and a [SphereFile], produce a stream that yields the
/// file content as an HTML document
pub fn subtext_to_html_document_stream<T, R>(
    transform: T,
    file: SphereFile<R>,
) -> impl Stream<Item = String>
where
    T: Transform,
    R: AsyncRead + Unpin,
{
    stream! {
      let (html_prefix, html_suffix) = html_document_envelope(&file.memo);
      let fragment_stream = subtext_to_html_fragment_stream(file, transform);

      yield html_prefix;

      for await fragment_part in fragment_stream {
        yield fragment_part;
      }

      yield html_suffix;
    }
}
