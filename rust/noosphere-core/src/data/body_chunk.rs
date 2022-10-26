use anyhow::{anyhow, Result};
use cid::Cid;
use fastcdc::FastCDC;
use libipld_cbor::DagCborCodec;
use serde::{Deserialize, Serialize};

use noosphere_storage::interface::BlockStore;

pub const BODY_CHUNK_MAX_SIZE: usize = 1024 * 64 * 8; // ~.5mb/chunk worst case

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
    pub async fn store_bytes<S: BlockStore>(bytes: &[u8], store: &mut S) -> Result<Cid> {
        let chunks = FastCDC::new(
            bytes,
            fastcdc::MINIMUM_MIN,
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

        next_chunk_cid.ok_or(anyhow!("No CID; did you try to store zero bytes?"))
    }

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
}
