use anyhow::Result;
use async_stream::try_stream;
use bytes::Bytes;
use cid::Cid;
use futures_util::{sink::SinkExt, TryStreamExt};
use iroh_car::{CarHeader, CarReader, CarWriter};
use noosphere_common::ConditionalSend;
use std::io::{Error as IoError, ErrorKind as IoErrorKind};
use tokio::sync::mpsc::channel;
use tokio_stream::Stream;
use tokio_util::{
    io::{CopyToBytes, SinkWriter, StreamReader},
    sync::PollSender,
};

/// Takes a [Bytes] stream and interprets it as a
/// [CARv1](https://ipld.io/specs/transport/car/carv1/), returning a stream of
/// `(Cid, Vec<u8>)` blocks.
pub fn from_car_stream<S, E>(
    stream: S,
) -> impl Stream<Item = Result<(Cid, Vec<u8>)>> + ConditionalSend + 'static
where
    E: std::error::Error + Send + Sync + 'static,
    S: Stream<Item = Result<Bytes, E>> + ConditionalSend + 'static,
{
    let stream = stream.map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error));

    try_stream! {
        tokio::pin!(stream);

        let reader = CarReader::new(StreamReader::new(stream)).await?;
        let stream = reader.stream();

        tokio::pin!(stream);

        while let Some(entry) = tokio_stream::StreamExt::try_next(&mut stream).await? {
            yield entry;
        }
    }
}

/// Takes a list of roots and a stream of blocks (pairs of [Cid] and
/// corresponding [Vec<u8>]), and produces an async byte stream that yields a
/// valid [CARv1](https://ipld.io/specs/transport/car/carv1/)
pub fn to_car_stream<S>(
    mut roots: Vec<Cid>,
    block_stream: S,
) -> impl Stream<Item = Result<Bytes, IoError>> + ConditionalSend
where
    S: Stream<Item = Result<(Cid, Vec<u8>)>> + ConditionalSend,
{
    if roots.is_empty() {
        roots = vec![Cid::default()]
    }

    try_stream! {
        let (tx, mut rx) = channel::<Bytes>(16);
        let sink =
            PollSender::new(tx).sink_map_err(|error| {
                error!("Failed to send CAR frame: {}", error);
                IoError::from(IoErrorKind::BrokenPipe)
            });

        let mut car_buffer = SinkWriter::new(CopyToBytes::new(sink));
        let car_header = CarHeader::new_v1(roots);
        let mut car_writer = CarWriter::new(car_header, &mut car_buffer);
        let mut sent_blocks = false;

        for await item in block_stream {
            sent_blocks = true;
            let (cid, block) = item.map_err(|error| {
                error!("Failed to stream blocks: {}", error);
                IoError::from(IoErrorKind::BrokenPipe)
            })?;

            car_writer.write(cid, block).await.map_err(|error| {
                error!("Failed to write CAR frame: {}", error);
                IoError::from(IoErrorKind::BrokenPipe)
            })?;

            car_writer.flush().await.map_err(|error| {
                error!("Failed to flush CAR frames: {}", error);
                IoError::from(IoErrorKind::BrokenPipe)
            })?;

            while let Ok(block) = rx.try_recv() {
                yield block;
            }
       }

       if !sent_blocks {
            car_writer.write_header().await.map_err(|error| {
                error!("Failed to write CAR frame: {}", error);
                IoError::from(IoErrorKind::BrokenPipe)
            })?;
            car_writer.flush().await.map_err(|error| {
                error!("Failed to flush CAR frames: {}", error);
                IoError::from(IoErrorKind::BrokenPipe)
            })?;

            while let Ok(block) = rx.try_recv() {
                yield block;
            }
       }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use async_stream::try_stream;
    use cid::Cid;
    use futures_util::Stream;
    use libipld_cbor::DagCborCodec;
    use noosphere_common::helpers::TestEntropy;
    use noosphere_storage::{BlockStore, MemoryStorage, Storage};

    use rand::Rng;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    use crate::stream::{from_car_stream, put_block_stream, to_car_stream};

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_converts_block_streams_to_car_streams_and_back() -> Result<()> {
        let test_entropy = TestEntropy::default();
        let rng_one = test_entropy.to_rng();
        let storage = MemoryStorage::default();
        let store_one = storage.get_block_store("one").await?;
        let store_two = storage.get_block_store("two").await?;

        let block_stream = {
            let mut store_one = store_one.clone();
            try_stream! {
                for _ in 0..10 {
                    let raw_bytes = rng_one.lock().await.gen::<[u8; 32]>();
                    let block_cid = store_one.save::<DagCborCodec, _>(raw_bytes.as_ref()).await?;
                    let block_bytes = store_one.require_block(&block_cid).await?;

                    yield (block_cid, block_bytes);
                }
            }
        };
        // See: https://github.com/tokio-rs/async-stream/issues/33#issuecomment-1261435381
        let _: &dyn Stream<Item = Result<(Cid, Vec<u8>)>> = &block_stream;

        let car_stream = to_car_stream(vec![], block_stream);

        let output_block_stream = from_car_stream(car_stream);

        put_block_stream(store_two.clone(), output_block_stream).await?;

        assert_eq!(store_one.entries.lock().await.len(), 10);
        store_one.expect_replica_in(&store_two).await?;

        Ok(())
    }
}
