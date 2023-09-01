use anyhow::Result;
use cid::Cid;
use libipld_cbor::DagCborCodec;
use libipld_core::raw::RawCodec;
use noosphere_storage::BlockStore;
use tokio_stream::{Stream, StreamExt};

/// Helper to put blocks from a [Stream] into any implementor of [BlockStore]
///
/// Implementation note: this is a stand-alone helper because defining this
/// async function on a trait (necessitating use of `#[async_trait]`) creates an
/// unergonomic `Send` bound on returned [Future].
pub async fn put_block_stream<S, Str>(mut store: S, stream: Str) -> Result<()>
where
    S: BlockStore,
    Str: Stream<Item = Result<(Cid, Vec<u8>)>>,
{
    tokio::pin!(stream);

    let mut stream_count = 0usize;

    while let Some((cid, block)) = stream.try_next().await? {
        stream_count += 1;
        trace!(?cid, "Putting streamed block {stream_count}...");

        store.put_block(&cid, &block).await?;

        match cid.codec() {
            codec_id if codec_id == u64::from(DagCborCodec) => {
                store.put_links::<DagCborCodec>(&cid, &block).await?;
            }
            codec_id if codec_id == u64::from(RawCodec) => {
                store.put_links::<RawCodec>(&cid, &block).await?;
            }
            codec_id => warn!("Unrecognized codec {}; skipping...", codec_id),
        }
    }

    trace!("Successfully put {stream_count} blocks from stream...");

    Ok(())
}
