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

    // TODO(#263): Consider using async-stream crate for this
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

#[cfg(test)]
mod tests {
    use cid::Cid;
    use noosphere_storage::MemoryStore;
    use ucan::crypto::KeyMaterial;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    use crate::{
        authority::generate_ed25519_key,
        data::MemoIpld,
        view::{Sphere, SphereMutation},
    };

    use super::Timeline;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_includes_the_revision_delimiters() {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (mut sphere, ucan, _) = Sphere::generate(&owner_did, &mut store).await.unwrap();
        let mut lineage = vec![*sphere.cid()];

        for i in 0..5u8 {
            let mut mutation = SphereMutation::new(&owner_did);
            mutation.links_mut().set(
                &format!("foo/{i}"),
                &MemoIpld::for_body(&mut store, &[i]).await.unwrap(),
            );
            let mut revision = sphere.apply_mutation(&mutation).await.unwrap();
            let next_cid = revision.try_sign(&owner_key, Some(&ucan)).await.unwrap();

            sphere = Sphere::at(&next_cid, &store);
            lineage.push(next_cid);
        }

        let past = lineage[1];
        let future = lineage[3];

        let timeline = Timeline::new(&store);
        let timeslice = timeline.slice(&future, Some(&past));

        let items: Vec<Cid> = timeslice
            .try_to_chronological()
            .await
            .unwrap()
            .into_iter()
            .map(|(cid, _)| cid)
            .collect();

        assert_eq!(items.len(), 3);

        assert_eq!(items[0], past);
        assert_eq!(items[2], future);
    }
}
