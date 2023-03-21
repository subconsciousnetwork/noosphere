use anyhow::Result;
use cid::Cid;
use libipld_cbor::DagCborCodec;
use noosphere_storage::BlockStore;
use tokio::sync::OnceCell;

use crate::{
    data::AuthorityIpld,
    view::{AllowedUcans, RevokedUcans},
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

    pub async fn try_at_or_empty(cid: Option<&Cid>, store: &mut S) -> Result<Authority<S>> {
        Ok(match cid {
            Some(cid) => Self::at(cid, store),
            None => Self::try_empty(store).await?,
        })
    }

    pub async fn try_empty(store: &mut S) -> Result<Self> {
        let ipld = AuthorityIpld::try_empty(store).await?;
        let cid = store.save::<DagCborCodec, _>(ipld).await?;

        Ok(Authority {
            cid,
            store: store.clone(),
            body: OnceCell::new(),
        })
    }

    pub async fn try_get_allowed_ucans(&self) -> Result<AllowedUcans<S>> {
        let ipld = self.to_body().await?;

        AllowedUcans::at_or_empty(Some(&ipld.allowed), &mut self.store.clone()).await
    }

    pub async fn try_get_revoked_ucans(&self) -> Result<RevokedUcans<S>> {
        let ipld = self.to_body().await?;

        RevokedUcans::at_or_empty(Some(&ipld.revoked), &mut self.store.clone()).await
    }
}
