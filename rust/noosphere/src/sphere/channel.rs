use std::{marker::PhantomData, sync::Arc};

use noosphere_sphere::{HasMutableSphereContext, HasSphereContext, SphereContext};
use noosphere_storage::Storage;
use tokio::sync::Mutex;
use ucan::crypto::KeyMaterial;

/// A [SphereChannel] provides duplex access to a given sphere, where one side
/// is read-only/immutable, and the other side is read-write/mutable. This
/// supports immutable sphere reads to happen in parallel with long-running
/// mutable operations on the sphere. Note that it is up to the user of a
/// [SphereChannel] to be mindful of potential race conditions. For example, if
/// you read from the immutable side while concurrently writing to the mutable
/// side, the result of your read will subject to the race outcome.
#[derive(Clone)]
pub struct SphereChannel<K, S, Ci, Cm>
where
    K: KeyMaterial + Clone + 'static,
    S: Storage,
    Ci: HasSphereContext<K, S>,
    Cm: HasMutableSphereContext<K, S>,
{
    immutable: Ci,
    mutable: Cm,
    key_marker: PhantomData<K>,
    storage_marker: PhantomData<S>,
}

impl<K, S, Ci, Cm> SphereChannel<K, S, Ci, Cm>
where
    K: KeyMaterial + Clone + 'static,
    S: Storage,
    Ci: HasSphereContext<K, S>,
    Cm: HasMutableSphereContext<K, S>,
{
    pub fn new(immutable: Ci, mutable: Cm) -> Self {
        Self {
            immutable,
            mutable,
            key_marker: PhantomData,
            storage_marker: PhantomData,
        }
    }

    /// The immutable / read-only side of the channel
    pub fn immutable(&self) -> &Ci {
        &self.immutable
    }

    /// The mutable / read-write side of the channel
    pub fn mutable(&mut self) -> &mut Cm {
        &mut self.mutable
    }
}

impl<K, S> From<SphereContext<K, S>>
    for SphereChannel<K, S, Arc<SphereContext<K, S>>, Arc<Mutex<SphereContext<K, S>>>>
where
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    fn from(value: SphereContext<K, S>) -> Self {
        SphereChannel::new(Arc::new(value.clone()), Arc::new(Mutex::new(value)))
    }
}
