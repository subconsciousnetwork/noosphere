use std::{collections::VecDeque, fmt::Display};

use anyhow::Result;
use cid::Cid;
use futures::{stream, StreamExt, TryStream};

use crate::data::MemoIpld;

use noosphere_storage::interface::{DagCborStore, Store};

// Assumptions:
// - network operations are _always_ mediated by a "remote" agent (no client-to-client syncing)
// - the "remote" always has the authoritative state (we always rebase merge onto remote's tip)

pub struct Timeline<'a, Storage: Store> {
    pub store: &'a Storage,
}

impl<'a, Storage: Store> Timeline<'a, Storage> {
    pub fn new(store: &'a Storage) -> Self {
        Timeline { store }
    }

    pub fn slice(&'a self, future: &'a Cid, past: &'a Cid) -> Timeslice<'a, Storage> {
        Timeslice {
            timeline: self,
            past,
            future,
        }
    }

    pub fn try_stream(
        &self,
        future: &Cid,
        past: &Cid,
    ) -> impl TryStream<Item = Result<(Cid, MemoIpld)>> {
        stream::try_unfold(
            (Some(future.clone()), past.clone(), self.store.clone()),
            |(from, to, storage)| async move {
                match from {
                    Some(from) => {
                        let cid = from;
                        let next_dag: MemoIpld = storage.load(&cid).await?;
                        let from = match from == to {
                            true => None,
                            false => next_dag.parent.clone(),
                        };

                        Ok(Some(((cid, next_dag), (from, to, storage))))
                    }
                    None => Ok(None),
                }
            },
        )
    }

    // pub async fn try_rebase(&mut self, l)
}

pub struct Timeslice<'a, Storage: Store> {
    pub timeline: &'a Timeline<'a, Storage>,
    pub past: &'a Cid,
    pub future: &'a Cid,
}

impl<'a, Storage: Store> Timeslice<'a, Storage> {
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

impl<'a, Storage: Store> Display for Timeslice<'a, Storage> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Timeslice(Past: {}, Future: {})", self.past, self.future)
    }
}
