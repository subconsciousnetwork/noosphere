use std::{collections::VecDeque, fmt::Display};

use anyhow::Result;
use cid::Cid;
use futures::{stream, StreamExt, TryStream};
use libipld_cbor::DagCborCodec;

use crate::data::MemoIpld;

use noosphere_storage::BlockStore;

// Assumptions:
// - network operations are _always_ mediated by a "remote" agent (no client-to-client syncing)
// - the "remote" always has the authoritative state (we always rebase merge onto remote's tip)

pub struct Timeline<'a, S: BlockStore> {
    pub store: &'a S,
}

impl<'a, S: BlockStore> Timeline<'a, S> {
    pub fn new(store: &'a S) -> Self {
        Timeline { store }
    }

    pub fn slice(&'a self, future: &'a Cid, past: Option<&'a Cid>) -> Timeslice<'a, S> {
        Timeslice {
            timeline: self,
            past,
            future,
        }
    }

    // TODO: Consider using async-stream crate for this
    pub fn try_stream(
        &self,
        future: &Cid,
        past: Option<&Cid>,
    ) -> impl TryStream<Item = Result<(Cid, MemoIpld)>> {
        stream::try_unfold(
            (Some(*future), past.cloned(), self.store.clone()),
            |(from, to, storage)| async move {
                match from {
                    Some(from) => {
                        let cid = from;
                        let next_dag = storage.load::<DagCborCodec, MemoIpld>(&cid).await?;

                        let next_from = match to {
                            Some(to) if from == to => None,
                            _ => next_dag.parent,
                        };

                        Ok(Some(((cid, next_dag), (next_from, to, storage))))
                    }
                    None => Ok(None),
                }
            },
        )
    }
}

pub struct Timeslice<'a, S: BlockStore> {
    pub timeline: &'a Timeline<'a, S>,
    pub past: Option<&'a Cid>,
    pub future: &'a Cid,
}

impl<'a, S: BlockStore> Timeslice<'a, S> {
    pub fn try_stream(&self) -> impl TryStream<Item = Result<(Cid, MemoIpld)>> {
        self.timeline.try_stream(self.future, self.past)
    }

    pub async fn try_to_chronological(&self) -> Result<Vec<(Cid, MemoIpld)>> {
        let mut chronological = VecDeque::new();
        let mut stream = Box::pin(self.try_stream());

        while let Some(result) = stream.next().await {
            chronological.push_front(result?);
        }

        Ok(chronological.into())
    }
}

impl<'a, S: BlockStore> Display for Timeslice<'a, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Timeslice(Past: {:?}, Future: {:?})",
            self.past, self.future
        )
    }
}
