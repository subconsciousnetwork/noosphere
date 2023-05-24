use anyhow::Result;
use async_stream::try_stream;
use bytes::Bytes;
use cid::Cid;
use futures_util::sink::SinkExt;
use noosphere_car::{CarHeader, CarWriter};
use std::io::{Error as IoError, ErrorKind as IoErrorKind};
use tokio::sync::mpsc::channel;
use tokio_stream::Stream;
use tokio_util::{
    io::{CopyToBytes, SinkWriter},
    sync::PollSender,
};

pub fn car_stream<S>(
    mut roots: Vec<Cid>,
    block_stream: S,
) -> impl Stream<Item = Result<Bytes, IoError>> + Send
where
    S: Stream<Item = Result<(Cid, Vec<u8>)>> + Send,
{
    if roots.len() == 0 {
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
