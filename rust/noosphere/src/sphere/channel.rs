use std::{marker::PhantomData, sync::Arc};

use noosphere_sphere::{HasMutableSphereContext, HasSphereContext, SphereContext, SphereCursor};
use noosphere_storage::Storage;
use tokio::sync::Mutex;

/// A [SphereChannel] provides duplex access to a given sphere, where one side
/// is read-only/immutable, and the other side is read-write/mutable. This
/// supports immutable sphere reads to happen in parallel with long-running
/// mutable operations on the sphere. Note that it is up to the user of a
/// [SphereChannel] to be mindful of potential race conditions. For example, if
/// you read from the immutable side while concurrently writing to the mutable
/// side, the result of your read will subject to the race outcome.
#[derive(Clone)]
pub struct SphereChannel<S, Ci, Cm>
where
    S: Storage,
    Ci: HasSphereContext<S>,
    Cm: HasMutableSphereContext<S>,
{
    immutable: Ci,
    mutable: Cm,
    storage_marker: PhantomData<S>,
}

impl<S, Ci, Cm> SphereChannel<S, Ci, Cm>
where
    S: Storage,
    Ci: HasSphereContext<S>,
    Cm: HasMutableSphereContext<S>,
{
    pub fn new(immutable: Ci, mutable: Cm) -> Self {
        Self {
            immutable,
            mutable,
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

impl<S> From<SphereContext<S>>
    for SphereChannel<S, Arc<SphereContext<S>>, Arc<Mutex<SphereContext<S>>>>
where
    S: Storage + 'static,
{
    fn from(value: SphereContext<S>) -> Self {
        SphereChannel::new(Arc::new(value.clone()), Arc::new(Mutex::new(value)))
    }
}

impl<S> From<SphereCursor<Arc<SphereContext<S>>, S>>
    for SphereChannel<S, SphereCursor<Arc<SphereContext<S>>, S>, Arc<Mutex<SphereContext<S>>>>
where
    S: Storage + 'static,
{
    fn from(value: SphereCursor<Arc<SphereContext<S>>, S>) -> Self {
        let mutable = Arc::new(Mutex::new(value.clone().to_inner().as_ref().clone()));

        SphereChannel::new(value, mutable)
    }
}
