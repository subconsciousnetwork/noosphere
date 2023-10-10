use anyhow::Result;
use async_stream::try_stream;
use futures_util::Stream;
use noosphere_common::ConditionalSend;
use std::{collections::BTreeSet, sync::Arc};
use tokio::sync::Mutex;

use cid::Cid;
use libipld_cbor::DagCborCodec;
use libipld_core::{codec::Codec, ipld::Ipld, raw::RawCodec};

/// A utility to help with tracking the relationship of blocks and references
/// within a series of blocks.
#[derive(Default, Clone, Debug)]
pub struct BlockLedger {
    references: BTreeSet<Cid>,
    blocks: BTreeSet<Cid>,
}

impl BlockLedger {
    /// Record a block in the ledger, extracting references from the block bytes
    /// and noting the block's own [Cid]
    pub fn record(&mut self, cid: &Cid, block: &[u8]) -> Result<()> {
        self.blocks.insert(*cid);

        match cid.codec() {
            codec if codec == u64::from(DagCborCodec) => {
                DagCborCodec.references::<Ipld, _>(block, &mut self.references)?;
            }
            codec if codec == u64::from(RawCodec) => {
                RawCodec.references::<Ipld, _>(block, &mut self.references)?;
            }
            _ => (),
        };

        Ok(())
    }

    /// Get an iterator over the [Cid]s of orphan blocks based on the current
    /// state of the [BlockLedger].
    ///
    /// Orphan blocks are blocks that are not referenced by any other blocks
    /// in the set of blocks recorded by this [BlockLedger].
    pub fn orphans(&self) -> impl IntoIterator<Item = &Cid> {
        self.blocks.difference(&self.references)
    }

    /// Same as [BlockLedger::orphans], but consumes the [BlockLedger] and
    /// yields owned [Cid]s.
    pub fn into_orphans(self) -> impl IntoIterator<Item = Cid> {
        self.blocks
            .into_iter()
            .filter(move |cid| !self.references.contains(cid))
    }

    /// Get an iterator over [Cid]s that are referenced by the recorded blocks
    /// but have not been recorded by this [BlockLedger] themselves.
    pub fn missing_references(&self) -> impl IntoIterator<Item = &Cid> {
        self.references
            .iter()
            .filter(|cid| !self.blocks.contains(*cid))
    }

    /// Same as [BlockLedger::missing_references], but consumes the
    /// [BlockLedger] and yields owned [Cid]s.
    pub fn into_missing_references(self) -> impl IntoIterator<Item = Cid> {
        self.references
            .into_iter()
            .filter(move |cid| !self.blocks.contains(cid))
    }
}

/// Wraps a block stream (any stream yielding `Result<(Cid, Vec<u8>)>`), and
/// records "orphan" blocks to a providede target buffer.
///
/// Orphan blocks are blocks that occurred in the stream but were not referenced
/// by any other blocks in the stream.
pub fn record_stream_orphans<E, S>(
    orphans: Arc<Mutex<E>>,
    block_stream: S,
) -> impl Stream<Item = Result<(Cid, Vec<u8>)>> + ConditionalSend
where
    E: Extend<Cid> + ConditionalSend,
    S: Stream<Item = Result<(Cid, Vec<u8>)>> + ConditionalSend,
{
    try_stream! {
        let mut ledger = BlockLedger::default();
        let mut locked_orphans = orphans.lock().await;

        for await item in block_stream {
          let (cid, block) = item?;

          ledger.record(&cid, &block)?;

          yield (cid, block);
        }

        locked_orphans.extend(ledger.into_orphans());
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, sync::Arc};

    use anyhow::Result;
    use cid::Cid;
    use futures_util::StreamExt;
    use tokio::sync::Mutex;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    use crate::{
        authority::Access,
        context::{HasMutableSphereContext, HasSphereContext, SpherePetnameWrite},
        helpers::{make_valid_link_record, simulated_sphere_context},
        stream::{memo_body_stream, record_stream_orphans},
    };

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_records_orphans_in_a_stream() -> Result<()> {
        let (mut sphere_context, _) = simulated_sphere_context(Access::ReadWrite, None).await?;
        let mut db = sphere_context.sphere_context().await?.db().clone();
        let (did, link_record, _) = make_valid_link_record(&mut db).await?;

        sphere_context.set_petname("foo", Some(did)).await?;
        sphere_context.save(None).await?;
        sphere_context
            .set_petname_record("foo", &link_record)
            .await?;

        let version_to_stream = sphere_context.save(None).await?;
        let orphans: Arc<Mutex<BTreeSet<Cid>>> = Default::default();

        let stream = record_stream_orphans(
            orphans.clone(),
            memo_body_stream(db.clone(), &version_to_stream, true),
        );

        tokio::pin!(stream);

        let _ = stream.collect::<Vec<Result<(Cid, Vec<u8>)>>>().await;

        let orphans = orphans.lock().await;

        assert_eq!(orphans.len(), 2);

        let root = sphere_context.version().await?;

        assert!(orphans.contains(&root));

        let orphan_link_record = Cid::try_from(
            link_record
                .proofs()
                .as_ref()
                .unwrap()
                .first()
                .unwrap()
                .as_str(),
        )?;

        assert!(orphans.contains(&orphan_link_record));

        Ok(())
    }
}
