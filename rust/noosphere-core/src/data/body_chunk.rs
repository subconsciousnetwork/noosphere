use crate::stream::reverse_stream;
use anyhow::{anyhow, Result};
use async_stream::try_stream;
use bytes::Bytes;
use cid::Cid;
use fastcdc::v2020::{AsyncStreamCDC, FastCDC};
use libipld_cbor::DagCborCodec;
use noosphere_storage::{BlockStore, Scratch};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncRead;
use tokio_stream::{Stream, StreamExt};

pub const BODY_CHUNK_MAX_SIZE: u32 = 1024 * 1024; // ~1mb/chunk worst case, ~.5mb/chunk average case
/// Encoding content larger than `CONTENT_STORAGE_MEMORY_LIMIT` will
/// use disk-storage rather than memory storage.
pub const CONTENT_STORAGE_MEMORY_LIMIT: u64 = 1024 * 1024 * 5; // 5mb

/// A body chunk is a simplified flexible byte layout used for linking
/// chunks of bytes. This is necessary to support cases when body contents
/// byte size exceeds the IPFS block size (~1MB). This may be replaced with
/// a more sophisticated layout structure in the future.
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub struct BodyChunkIpld {
    /// A chunk of bytes
    pub bytes: Vec<u8>,
    /// An optional pointer to the next chunk of bytes, if there are any remaining
    pub next: Option<Cid>,
}

impl BodyChunkIpld {
    #[deprecated(note = "Use `BodyChunkIpld::encode` instead for a streaming interface")]
    pub async fn store_bytes<S: BlockStore>(bytes: &[u8], store: &mut S) -> Result<Cid> {
        let chunks = FastCDC::new(
            bytes,
            fastcdc::v2020::MINIMUM_MIN,
            BODY_CHUNK_MAX_SIZE / 2,
            BODY_CHUNK_MAX_SIZE,
        );
        let mut byte_chunks = Vec::new();

        for chunk in chunks {
            let length = chunk.length;
            let offset = chunk.offset;
            let end = offset + length;
            let bytes = &bytes[offset..end];

            byte_chunks.push(bytes);
        }

        let mut next_chunk_cid = None;

        for byte_chunk in byte_chunks.into_iter().rev() {
            next_chunk_cid = Some(
                store
                    .save::<DagCborCodec, _>(&BodyChunkIpld {
                        bytes: Vec::from(byte_chunk),
                        next: next_chunk_cid,
                    })
                    .await?,
            );
        }

        next_chunk_cid.ok_or_else(|| anyhow!("No CID; did you try to store zero bytes?"))
    }

    #[deprecated(note = "Use `BodyChunkIpld::decode` instead for a streaming interface")]
    pub async fn load_all_bytes<S: BlockStore>(&self, store: &S) -> Result<Vec<u8>> {
        let mut all_bytes = self.bytes.clone();
        let mut next_cid = self.next;

        while let Some(cid) = next_cid {
            let BodyChunkIpld { mut bytes, next } = store.load::<DagCborCodec, _>(&cid).await?;

            all_bytes.append(&mut bytes);
            next_cid = next;
        }

        Ok(all_bytes)
    }

    /// Encode `content` as a [BodyChunkIpld] chain in streaming fashion,
    /// returning a [Stream] that yields a [Cid] and [BodyChunkIpld] tuple
    /// in reverse order. `buffer_strategy` of `None` applies defaults.
    pub async fn encode_streaming<'a, R, S>(
        content: R,
        store: &'a S,
        buffer_strategy: Option<BufferStrategy>,
    ) -> impl Stream<Item = Result<(Cid, BodyChunkIpld), anyhow::Error>> + Unpin + 'a
    where
        R: AsyncRead + Unpin + 'a,
        S: Scratch + BlockStore,
    {
        Box::pin(try_stream! {
            let mut chunker = AsyncStreamCDC::new(
                content,
                fastcdc::v2020::MINIMUM_MIN,
                BODY_CHUNK_MAX_SIZE / 2,
                BODY_CHUNK_MAX_SIZE,
            );
            let stream = chunker.as_stream().map(|chunk_data| chunk_data.map(|chunk_data| chunk_data.data));


            let memory_limit = if let Some(BufferStrategy::Limit(memory_limit)) = buffer_strategy {
                memory_limit
            } else {
                CONTENT_STORAGE_MEMORY_LIMIT
            };

            let stream = reverse_stream(stream, store, memory_limit);
            tokio::pin!(stream);

            let mut store = store.to_owned();
            let mut next_chunk_cid = None;
            while let Some(chunk) = stream.try_next().await? {
                let chunk = BodyChunkIpld {
                    bytes: chunk,
                    next: next_chunk_cid,
                };

                let cid = store.save::<DagCborCodec, _>(&chunk).await?;
                yield (cid, chunk);
                next_chunk_cid = Some(cid);
            }
        })
    }

    /// Encode `content` as a [BodyChunkIpld] chain in streaming fashion,
    /// returning the root [Cid] upon completion. `buffer_strategy` of `None`
    /// applies defaults.
    pub async fn encode<R, S>(
        content: R,
        store: &S,
        buffer_strategy: Option<BufferStrategy>,
    ) -> Result<Cid>
    where
        R: AsyncRead + Unpin,
        S: Scratch + BlockStore,
    {
        let stream = BodyChunkIpld::encode_streaming(content, store, buffer_strategy).await;
        tokio::pin!(stream);

        let mut head_cid = None;
        while let Some((cid, _)) = stream.try_next().await? {
            head_cid = Some(cid);
        }
        match head_cid {
            Some(cid) => Ok(cid),
            None => Err(anyhow!("Could not encode empty buffer.")),
        }
    }

    /// Decode a [BodyChunkIpld] chain via [Cid] into a [Bytes] stream.
    pub fn decode<S>(
        cid: &Cid,
        store: &S,
    ) -> impl Stream<Item = Result<Bytes, std::io::Error>> + Unpin
    where
        S: BlockStore,
    {
        let mut next = Some(*cid);
        let store = store.clone();
        Box::pin(try_stream! {
            while let Some(cid) = next {
                debug!("Unpacking block {}...", cid);
                let chunk = store.load::<DagCborCodec, BodyChunkIpld>(&cid).await.map_err(|error| {
                    std::io::Error::new(std::io::ErrorKind::UnexpectedEof, error.to_string())
                })?;
                yield Bytes::from(chunk.bytes);
                next = chunk.next;
            }
        })
    }
}

/// Buffering strategy needed to process IPLD encoding.
pub enum BufferStrategy {
    /// Buffers to store provider once limit in bytes is reached.
    Limit(u64),
}

#[cfg(test)]
mod tests {
    use super::*;
    use noosphere_storage::{helpers::make_disposable_storage, MemoryStore, SphereDb};
    use tokio_stream::StreamExt;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_reads_and_writes_chunks_nonstreaming() -> Result<()> {
        let chunk_size = 1024 * 1024;
        let mut store = MemoryStore::default();
        let chunk1 = vec![1; chunk_size.try_into().unwrap()];
        let chunk2 = vec![2; chunk_size.try_into().unwrap()];
        let chunk3 = vec![3; <u32 as TryInto::<usize>>::try_into(chunk_size).unwrap() / 2];
        let bytes = [chunk1.clone(), chunk2.clone(), chunk3.clone()].concat();

        #[allow(deprecated)]
        let cid = BodyChunkIpld::store_bytes(&bytes, &mut store).await?;
        let ipld_chunk = store.load::<DagCborCodec, BodyChunkIpld>(&cid).await?;

        #[allow(deprecated)]
        let output = ipld_chunk.load_all_bytes(&store).await?;
        assert_eq!(output, bytes);
        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_reads_and_writes_chunks_streaming() -> Result<()> {
        let chunk_size = 1024;
        let chunk_count = 10;
        let memory_limit = chunk_size * 5;
        let provider = make_disposable_storage().await?;
        let db = SphereDb::new(&provider).await?;
        let mut chunks = vec![];
        for n in 1..=chunk_count {
            let mut chunk: Vec<u8> = vec![0; chunk_size.try_into().unwrap()];
            chunk.fill(n);
            chunks.push(chunk);
        }
        let bytes = chunks.concat();
        assert!(bytes.len() as u64 > memory_limit);

        let cid = BodyChunkIpld::encode(
            bytes.as_ref(),
            &db,
            Some(BufferStrategy::Limit(memory_limit)),
        )
        .await?;

        let stream = BodyChunkIpld::decode(&cid, &db);
        drop(db);
        tokio::pin!(stream);
        let mut output: Vec<Vec<u8>> = vec![];
        while let Some(chunk) = stream.try_next().await? {
            output.push(chunk.into());
        }
        assert_eq!(output.concat(), bytes);
        Ok(())
    }
}
