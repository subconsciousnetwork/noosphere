use crate::data::BodyChunkIpld;
use async_stream::try_stream;
use bytes::Bytes;
use cid::Cid;
use libipld_cbor::DagCborCodec;
use noosphere_storage::BlockStore;
use tokio_stream::Stream;

/// Helper to easily decode a linked list of `BodyChunkIpld` as a byte stream
pub struct BodyChunkDecoder<'a, 'b, S: BlockStore>(pub &'a Cid, pub &'b S);

impl<'a, 'b, S: BlockStore> BodyChunkDecoder<'a, 'b, S> {
    /// Consume the [BodyChunkDecoder] and return an async [Stream] of bytes
    /// representing the raw body contents
    pub fn stream(self) -> impl Stream<Item = Result<Bytes, std::io::Error>> + Unpin {
        let mut next = Some(*self.0);
        let store = self.1.clone();
        Box::pin(try_stream! {
            while let Some(cid) = next {
                trace!("Unpacking block {}...", cid);
                let chunk = store.load::<DagCborCodec, BodyChunkIpld>(&cid).await.map_err(|error| {
                    std::io::Error::new(std::io::ErrorKind::UnexpectedEof, error.to_string())
                })?;
                yield Bytes::from(chunk.bytes);
                next = chunk.next;
            }
        })
    }
}
