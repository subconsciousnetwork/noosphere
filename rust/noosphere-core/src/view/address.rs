use std::ops::Deref;

use anyhow::Result;
use cid::Cid;
use libipld_cbor::DagCborCodec;
use noosphere_storage::BlockStore;
use tokio::sync::OnceCell;

use crate::data::AddressBookIpld;

use super::Identities;

/// A view over an [AddressBookIpld] that provides relatively high level access
/// to the underlying data. This view is mainly used as an intermediate point to
/// access the petname identities of a sphere.
pub struct AddressBook<S: BlockStore> {
    cid: Cid,
    store: S,
    body: OnceCell<AddressBookIpld>,
}

impl<S> AddressBook<S>
where
    S: BlockStore,
{
    pub fn cid(&self) -> &Cid {
        &self.cid
    }

    /// Initialize an [AddressBook] at the given [Cid] version; even though a
    /// version and [BlockStore] are provided, the initialized [AddressBook] is
    /// lazy and won't load the associated [AddressBookIpld] unless is is
    /// accessed.
    pub fn at(cid: &Cid, store: &S) -> Self {
        AddressBook {
            cid: *cid,
            store: store.clone(),
            body: OnceCell::new(),
        }
    }

    /// Loads the underlying IPLD (if it hasn't been loaded already) and returns
    /// an owned copy of it
    pub async fn to_body(&self) -> Result<AddressBookIpld> {
        Ok(self
            .body
            .get_or_try_init(|| async { self.store.load::<DagCborCodec, _>(self.cid()).await })
            .await?
            .clone())
    }

    /// Similar to [AddressBook::at], but initializes an empty [AddressBookIpld]
    /// if `None` is provided for the [Cid] version.
    pub async fn at_or_empty<C>(cid: Option<C>, store: &mut S) -> Result<AddressBook<S>>
    where
        C: Deref<Target = Cid>,
    {
        Ok(match cid {
            Some(cid) => Self::at(&cid, store),
            None => Self::empty(store).await?,
        })
    }

    /// Initializes an empty [AddressBookIpld] and persists it before
    /// intializing the [AddressBook] with the version [Cid] of the empty
    /// [AddressBookIpld].
    pub async fn empty(store: &mut S) -> Result<Self> {
        let ipld = AddressBookIpld::empty(store).await?;
        let cid = store.save::<DagCborCodec, _>(ipld).await?;

        Ok(AddressBook {
            cid,
            store: store.clone(),
            body: OnceCell::new(),
        })
    }

    /// Get a [Identities] view over the petname identities in the sphere
    pub async fn get_identities(&self) -> Result<Identities<S>> {
        let ipld = self.to_body().await?;

        Ok(Identities::at(&ipld.identities, &self.store.clone()))
    }
}
