use std::ops::Deref;

use anyhow::Result;
use cid::Cid;
use libipld_cbor::DagCborCodec;
use noosphere_storage::BlockStore;
use tokio::sync::OnceCell;

use crate::{
    data::AuthorityIpld,
    view::{Delegations, Revocations},
};

/// A view in to the authorizations (and associated revocations) that pertain
/// to sphere access
pub struct Authority<S: BlockStore> {
    cid: Cid,
    store: S,
    body: OnceCell<AuthorityIpld>,
}

impl<S> Authority<S>
where
    S: BlockStore,
{
    pub fn cid(&self) -> &Cid {
        &self.cid
    }

    pub fn at(cid: &Cid, store: &S) -> Self {
        Authority {
            cid: *cid,
            store: store.clone(),
            body: OnceCell::new(),
        }
    }

    /// Loads the underlying IPLD (if it hasn't been loaded already) and returns
    /// an owned copy of it
    pub async fn to_body(&self) -> Result<AuthorityIpld> {
        Ok(self
            .body
            .get_or_try_init(|| async { self.store.load::<DagCborCodec, _>(self.cid()).await })
            .await?
            .clone())
    }

    pub async fn at_or_empty<C>(cid: Option<C>, store: &mut S) -> Result<Authority<S>>
    where
        C: Deref<Target = Cid>,
    {
        Ok(match cid {
            Some(cid) => Self::at(&cid, store),
            None => Self::empty(store).await?,
        })
    }

    pub async fn empty(store: &mut S) -> Result<Self> {
        let ipld = AuthorityIpld::empty(store).await?;
        let cid = store.save::<DagCborCodec, _>(ipld).await?;

        Ok(Authority {
            cid,
            store: store.clone(),
            body: OnceCell::new(),
        })
    }

    pub async fn get_delegations(&self) -> Result<Delegations<S>> {
        let ipld = self.to_body().await?;

        Delegations::at_or_empty(Some(ipld.delegations), &mut self.store.clone()).await
    }

    pub async fn get_revocations(&self) -> Result<Revocations<S>> {
        let ipld = self.to_body().await?;

        Revocations::at_or_empty(Some(ipld.revocations), &mut self.store.clone()).await
    }
}
