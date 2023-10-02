use std::{collections::VecDeque, fmt::Display};

use anyhow::Result;
use futures::{stream, StreamExt, TryStream};
use libipld_cbor::DagCborCodec;

use crate::data::{Link, MemoIpld};

use noosphere_storage::BlockStore;

// Assumptions:
// - network operations are _always_ mediated by a "remote" agent (no client-to-client syncing)
// - the "remote" always has the authoritative state (we always rebase merge onto remote's tip)

#[cfg(doc)]
use tokio_stream::Stream;

/// A helper for turning contiguous ranges of [Link<MemoIpld>]s into [Timeslice]s.
#[derive(Debug)]
pub struct Timeline<'a, S: BlockStore> {
    /// The [BlockStore] that will be shared with any [Timeslice]s produced by
    /// this [Timeline]
    pub store: &'a S,
}

impl<'a, S: BlockStore> Timeline<'a, S> {
    /// Initialize a new [Timeline] with a backing [BlockStore]
    pub fn new(store: &'a S) -> Self {
        Timeline { store }
    }

    /// Produce a [Timeslice], which represents a reverse-chronological series
    /// of [Link<MemoIpld>] that occur between a specified bounds.
    pub fn slice(
        &'a self,
        future: &'a Link<MemoIpld>,
        past: Option<&'a Link<MemoIpld>>,
    ) -> Timeslice<'a, S> {
        Timeslice {
            timeline: self,
            past,
            future,
            exclude_past: false,
        }
    }

    // TODO(#263): Consider using async-stream crate for this
    // TODO(tokio-rs/tracing#2503): instrument + impl trait causes clippy
    // warning
    /// Produce a [TryStream] whose items are a series of [(Link<MemoIpld>,
    /// MemoIpld)], each one the ancestor of the last, yielded in
    /// reverse-chronological order.
    #[allow(clippy::let_with_type_underscore)]
    #[instrument(level = "trace", skip(self))]
    pub fn stream(
        &self,
        future: &Link<MemoIpld>,
        past: Option<&Link<MemoIpld>>,
        exclude_past: bool,
    ) -> impl TryStream<Item = Result<(Link<MemoIpld>, MemoIpld)>> {
        stream::try_unfold(
            (Some(*future), past.cloned(), self.store.clone()),
            move |(from, to, storage)| async move {
                match &from {
                    Some(from) => {
                        trace!("Stepping backward through version {}...", from);
                        let cid = from;
                        let next_dag = storage.load::<DagCborCodec, MemoIpld>(cid).await?;

                        let next_from: Option<Link<MemoIpld>> = match &to {
                            Some(to) if from == to => None,
                            _ => {
                                if exclude_past && to.as_ref() == next_dag.parent.as_ref() {
                                    None
                                } else {
                                    next_dag.parent
                                }
                            }
                        };

                        Ok(Some(((*cid, next_dag), (next_from, to, storage))))
                    }
                    None => Ok(None),
                }
            },
        )
    }
}

/// A [Timeslice] represents a bounded chronological range of [Link<MemoIpld>]
/// within a [Timeline].
#[derive(Debug)]
pub struct Timeslice<'a, S: BlockStore> {
    /// The associated [Timeline] of this [Timeslice]
    pub timeline: &'a Timeline<'a, S>,
    /// The bound in the chronological "past," e.g., the earliest version;
    /// `None` means "the (inclusive) beginning"
    pub past: Option<&'a Link<MemoIpld>>,
    /// The bound in the chronological "future" e.g.,  the most recent version
    pub future: &'a Link<MemoIpld>,
    /// Whether or not to exclude the configured `past` from any iteration over
    /// the series of versions
    pub exclude_past: bool,
}

impl<'a, S: BlockStore> Timeslice<'a, S> {
    /// Produce a [TryStream] from this [Timeslice] that yields sphere versions
    /// and their memos in reverse-chronological order
    pub fn stream(&self) -> impl TryStream<Item = Result<(Link<MemoIpld>, MemoIpld)>> {
        self.timeline
            .stream(self.future, self.past, self.exclude_past)
    }

    /// Configure the [Timeslice] to be inclusive of the `past` bound
    pub fn include_past(mut self) -> Self {
        self.exclude_past = false;
        self
    }

    /// Configure the [Timeslice] to be exclusive of the `past` bound
    pub fn exclude_past(mut self) -> Self {
        self.exclude_past = true;
        self
    }

    /// Aggregate an array of versions in chronological order and return it;
    /// note that this can be quite memory costly (depending on how much history
    /// is being aggregated), so it is better to stream in reverse-chronological
    /// order if possible.
    pub async fn to_chronological(&self) -> Result<Vec<Link<MemoIpld>>> {
        let mut chronological = VecDeque::new();
        let mut stream = Box::pin(self.stream());

        while let Some(result) = stream.next().await {
            chronological.push_front(result?.0);
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
    use anyhow::Result;
    use libipld_cbor::DagCborCodec;
    use noosphere_storage::{BlockStore, MemoryStore};
    use ucan::crypto::KeyMaterial;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    use crate::{
        authority::generate_ed25519_key,
        data::{Link, MemoIpld},
        view::{Sphere, SphereMutation},
    };

    use super::Timeline;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_only_yields_one_item_when_past_equals_present() -> Result<()> {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await?;

        let (mut sphere, ucan, _) = Sphere::generate(&owner_did, &mut store).await?;
        let mut lineage = vec![*sphere.cid()];

        for i in 0..5u8 {
            let mut mutation = SphereMutation::new(&owner_did);
            let memo = MemoIpld::for_body(&mut store, &[i]).await?;
            let cid = store.save::<DagCborCodec, _>(&memo).await?;

            mutation.content_mut().set(&format!("foo/{i}"), &cid.into());
            let mut revision = sphere.apply_mutation(&mutation).await?;
            let next_cid = revision.sign(&owner_key, Some(&ucan)).await?;

            sphere = Sphere::at(&next_cid, &store);
            lineage.push(next_cid);
        }

        let past = lineage[4];
        let future = lineage[4];

        let timeline = Timeline::new(&store);
        let timeslice = timeline.slice(&future, Some(&past));

        let items: Vec<Link<MemoIpld>> = timeslice.to_chronological().await?;

        assert_eq!(items.len(), 1);

        assert_eq!(items[0], past);
        assert_eq!(items[0], future);

        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_includes_the_revision_delimiters() -> Result<()> {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await?;

        let (mut sphere, ucan, _) = Sphere::generate(&owner_did, &mut store).await?;
        let mut lineage = vec![*sphere.cid()];

        for i in 0..5u8 {
            let mut mutation = SphereMutation::new(&owner_did);
            let memo = MemoIpld::for_body(&mut store, &[i]).await?;
            let cid = store.save::<DagCborCodec, _>(&memo).await?;

            mutation.content_mut().set(&format!("foo/{i}"), &cid.into());
            let mut revision = sphere.apply_mutation(&mutation).await?;
            let next_cid = revision.sign(&owner_key, Some(&ucan)).await?;

            sphere = Sphere::at(&next_cid, &store);
            lineage.push(next_cid);
        }

        let past = lineage[1];
        let future = lineage[3];

        let timeline = Timeline::new(&store);
        let timeslice = timeline.slice(&future, Some(&past));

        let items: Vec<Link<MemoIpld>> = timeslice.to_chronological().await?;

        assert_eq!(items.len(), 3);

        assert_eq!(items[0], past);
        assert_eq!(items[2], future);

        Ok(())
    }
}
