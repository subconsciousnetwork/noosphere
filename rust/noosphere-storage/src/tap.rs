use crate::BlockStore;
use anyhow::Result;
use async_trait::async_trait;
use cid::Cid;
use tokio::sync::mpsc::{channel, Receiver, Sender};

/// Instruments a [BlockStore], sending a copy of each block read to a [Receiver].
///
/// This allows an observer to record all the blocks needed to load arbitrarily deep
/// and complex DAGs into memory without orchestrating a dedicated callback for the
/// DAG implementations to invoke.
///
/// Note that the [Receiver] end of the channel will consider the channel open
/// (and thus will continue to await values) until all of its associated
/// [BlockStoreTap] clones are dropped. If you expect a finite number of blocks
/// to be sent to the [Receiver], ensure that all [BlockStoreTap] clones are
/// eventually dropped. Otherwise, the [Receiver] will continue waiting to
/// receive blocks.
#[derive(Clone)]
pub struct BlockStoreTap<S>
where
    S: BlockStore,
{
    store: S,
    tx: Sender<(Cid, Vec<u8>)>,
}

impl<S> BlockStoreTap<S>
where
    S: BlockStore,
{
    /// Wraps a [BlockStore], setting the channel capacity to `capacity`, returning
    /// the wrapped store and [Receiver].
    pub fn new(store: S, capacity: usize) -> (Self, Receiver<(Cid, Vec<u8>)>) {
        let (tx, rx) = channel(capacity);
        (BlockStoreTap { store, tx }, rx)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S> BlockStore for BlockStoreTap<S>
where
    S: BlockStore,
{
    async fn put_block(&mut self, cid: &Cid, block: &[u8]) -> Result<()> {
        self.store.put_block(cid, block).await
    }

    async fn get_block(&self, cid: &Cid) -> Result<Option<Vec<u8>>> {
        Ok(match self.store.get_block(cid).await? {
            Some(block) => {
                self.tx.send((*cid, block.clone())).await?;
                Some(block)
            }
            None => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use cid::Cid;
    use libipld_cbor::DagCborCodec;
    use tokio_stream::{wrappers::ReceiverStream, StreamExt};
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    use crate::{block_deserialize, BlockStore, BlockStoreTap, MemoryStore};

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_sends_all_retrieved_blocks_to_the_channel() {
        let store = MemoryStore::default();

        let (mut tap, mut rx) = BlockStoreTap::new(store, 32);

        let mut cids = Vec::new();

        for i in 0..10 {
            cids.push(tap.save::<DagCborCodec, _>(vec![i as u8]).await.unwrap());
        }

        assert_eq!(
            rx.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        );

        for cid in cids.iter() {
            tap.load::<DagCborCodec, Vec<u8>>(cid).await.unwrap();
        }

        drop(tap);

        let stream = ReceiverStream::new(rx);
        let results = stream.collect::<Vec<(Cid, Vec<u8>)>>().await;

        for (i, (cid, bytes)) in results.iter().enumerate() {
            assert_eq!(cid, &cids[i]);

            let value = block_deserialize::<DagCborCodec, Vec<u8>>(bytes).unwrap();

            assert_eq!(value.as_slice(), &[i as u8]);
        }
    }
}
