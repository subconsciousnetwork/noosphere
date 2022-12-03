use async_stream::stream;
use bytes::Bytes;
use futures::Stream;
use std::io::Error as IoError;
use tokio_util::io::StreamReader;

#[cfg(doc)]
use tokio::io::AsyncRead;

/// This is a helper for taking a [Stream] of strings and converting it
/// to an [AsyncRead] suitable for writing to a file.
pub struct TransformStream<S>(pub S)
where
    S: Stream<Item = String>;

impl<S> TransformStream<S>
where
    S: Stream<Item = String>,
{
    /// Consume the [TransformStream] and return a [StreamReader] that yields
    /// the stream as bytes.
    pub fn into_reader(self) -> StreamReader<impl Stream<Item = Result<Bytes, IoError>>, Bytes> {
        StreamReader::new(Box::pin(stream! {
            for await part in self.0 {
              yield Ok(Bytes::from(part));
            }
        }))
    }
}
