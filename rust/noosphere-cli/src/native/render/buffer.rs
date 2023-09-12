use anyhow::{anyhow, Result};
use cid::Cid;
use noosphere_core::context::{AsyncFileBody, SphereFile};
use noosphere_core::data::Did;
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    pin::Pin,
};
use tokio::task::JoinSet;

use super::SphereWriter;

/// The form a callback that can be passed when flushing a [ChangeBuffer]; note
/// that it is generic for the argument that is passed to the callback.
pub type ChangeBufferFlushCallback<T> =
    Box<dyn Fn(BTreeMap<String, T>, BTreeSet<String>) -> Pin<Box<dyn Future<Output = Result<()>>>>>;

/// A [ChangeBuffer] enables order-sensitive buffering of changes, and is meant
/// to be used when traversing incremental revisions of a sphere. If changes are
/// buffered in history order, they can be flushed and the flusher will be able
/// to work with a flattened representation of all those changes.
#[derive(Debug)]
pub struct ChangeBuffer<T> {
    capacity: usize,
    added: BTreeMap<String, T>,
    removed: BTreeSet<String>,
}

impl<T> ChangeBuffer<T> {
    /// Initialize a [ChangeBuffer] with the given capacity. When the capacity
    /// is reached, the [ChangeBuffer] _must_ be flushed before additional
    /// changes are buffered.
    pub fn new(capacity: usize) -> Self {
        ChangeBuffer {
            capacity,
            added: BTreeMap::default(),
            removed: BTreeSet::default(),
        }
    }

    fn assert_not_full(&self) -> Result<()> {
        if self.is_full() {
            Err(anyhow!("Change buffer is full"))
        } else {
            Ok(())
        }
    }

    /// Returns true if the [ChangeBuffer] is full
    pub fn is_full(&self) -> bool {
        (self.added.len() + self.removed.len()) >= self.capacity
    }

    /// Buffer an additive change by key
    pub fn add(&mut self, key: String, value: T) -> Result<()> {
        self.assert_not_full()?;

        self.removed.remove(&key);
        self.added.insert(key, value);

        Ok(())
    }

    /// Buffer a removal by key
    pub fn remove(&mut self, key: &str) -> Result<()> {
        self.assert_not_full()?;

        self.added.remove(key);
        self.removed.insert(key.to_owned());

        Ok(())
    }

    /// Take all the buffered, flattened changes. This has the effect of
    /// resetting the [ChangeBuffer] internally.
    pub fn take(&mut self) -> (BTreeMap<String, T>, BTreeSet<String>) {
        (
            std::mem::take(&mut self.added),
            std::mem::take(&mut self.removed),
        )
    }
}

impl<R> ChangeBuffer<SphereFile<R>>
where
    R: AsyncFileBody + 'static,
{
    /// Flush the [ChangeBuffer] to a [SphereWriter] for the case where we are
    /// dealing in sphere content
    #[instrument(skip(self))]
    pub async fn flush_to_writer(&mut self, writer: &SphereWriter) -> Result<()> {
        let (added, removed) = self.take();
        let mut changes = JoinSet::<Result<()>>::new();

        for (slug, mut file) in added {
            let writer = writer.clone();
            changes.spawn(async move {
                trace!("Writing '{slug}'...");
                writer.write_content(&slug, &mut file).await
            });
        }

        for slug in removed {
            let writer = writer.clone();
            changes.spawn(async move {
                trace!("Removing '{slug}'...");
                writer.remove_content(&slug).await
            });
        }

        while let Some(result) = changes.join_next().await {
            match result {
                Ok(result) => match result {
                    Ok(_) => (),
                    Err(error) => {
                        warn!("Content write failed: {}", error);
                    }
                },
                Err(error) => {
                    warn!("Content change task failed: {}", error);
                }
            };
        }

        Ok(())
    }
}

impl ChangeBuffer<(Did, Cid)> {
    /// Flush the [ChangeBuffer] to a [SphereWriter] for the case where we are
    /// dealing with peer references
    #[instrument]
    pub async fn flush_to_writer(&mut self, writer: &SphereWriter) -> Result<()> {
        let (added, removed) = self.take();
        let mut changes = JoinSet::<Result<()>>::new();

        for (petname, (did, cid)) in added {
            let writer = writer.clone();
            changes.spawn(async move {
                trace!("Writing @{petname}...");
                writer.symlink_peer(&did, &cid, &petname).await
            });
        }

        for petname in removed {
            let writer = writer.clone();
            changes.spawn(async move {
                trace!("Removing @{petname}...");
                writer.unlink_peer(&petname).await
            });
        }

        while let Some(result) = changes.join_next().await {
            match result {
                Ok(result) => match result {
                    Ok(_) => (),
                    Err(error) => {
                        warn!("Petname write failed: {}", error);
                    }
                },
                Err(error) => {
                    warn!("Petname change task failed: {}", error);
                }
            };
        }

        Ok(())
    }
}
