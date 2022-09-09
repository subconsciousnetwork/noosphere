use std::pin::Pin;

use anyhow::Result;
use async_once_cell::OnceCell;
use cid::Cid;
use futures::Stream;

use crate::{
    data::{LinksChangelogIpld, LinksIpld, MapOperation},
    view::LinksMutation,
};

use noosphere_collections::hamt::Hamt;
use noosphere_storage::interface::{DagCborStore, Store};

#[derive(Debug)]
pub struct Links<Storage: Store> {
    cid: Cid,
    store: Storage,
    // NOTE: OnceCell used here for the caching benefits; it may not be necessary for changelog
    hamt: OnceCell<Hamt<Storage, Cid, String>>,
    changelog: OnceCell<LinksChangelogIpld>,
}

impl<Storage: Store> Links<Storage> {
    pub async fn try_get_changelog(&self) -> Result<&LinksChangelogIpld> {
        self.changelog
            .get_or_try_init(async {
                let ipld: LinksIpld = self.store.load(&self.cid).await?;
                self.store.load(&ipld.changelog).await
            })
            .await
    }

    pub async fn try_get_hamt(&self) -> Result<&Hamt<Storage, Cid, String>> {
        self.hamt
            .get_or_try_init(async { self.try_load_hamt().await })
            .await
    }

    async fn try_load_hamt(&self) -> Result<Hamt<Storage, Cid, String>> {
        let ipld: LinksIpld = self.store.load(&self.cid).await?;
        ipld.try_load_hamt(&self.store).await
    }

    pub async fn try_at_or_empty(cid: Option<&Cid>, store: &mut Storage) -> Result<Links<Storage>> {
        Ok(match cid {
            Some(cid) => Links::at(&cid, store),
            None => Links::try_empty(store).await?,
        })
    }

    pub fn cid(&self) -> &Cid {
        &self.cid
    }

    pub fn at(cid: &Cid, store: &Storage) -> Links<Storage> {
        Links {
            cid: cid.clone(),
            store: store.clone(),
            hamt: OnceCell::new(),
            changelog: OnceCell::new(),
        }
    }

    pub async fn try_empty(store: &mut Storage) -> Result<Links<Storage>> {
        let ipld = LinksIpld::try_empty(store).await?;
        let cid = store.save(&ipld).await?;

        Ok(Links {
            cid,
            hamt: OnceCell::new(),
            changelog: OnceCell::new(),
            store: store.clone(),
        })
    }

    pub async fn get(&self, slug: &str) -> Result<Option<Cid>> {
        let links = self.try_get_hamt().await?;

        Ok(match links.get(&slug.to_string()).await? {
            Some(cid) => Some(cid.clone()),
            None => None,
        })
    }

    pub async fn try_apply_with_cid(
        cid: Option<&Cid>,
        mutation: &LinksMutation,
        store: &mut Storage,
    ) -> Result<Cid> {
        let links = Self::try_at_or_empty(cid, store).await?;
        let mut changelog = links.try_get_changelog().await?.mark(mutation.did());
        let mut hamt = links.try_load_hamt().await?;

        for change in mutation.changes() {
            match change {
                MapOperation::Add { key, value } => {
                    hamt.set(key.clone(), value.clone()).await?;
                }
                MapOperation::Remove { key } => {
                    hamt.delete(key).await?;
                }
            };

            changelog.push(change.clone())?;
        }

        let changelog_cid = store.save(&changelog).await?;
        let hamt_cid = hamt.flush().await?;
        let links_ipld = LinksIpld {
            hamt: hamt_cid,
            changelog: changelog_cid,
            ..Default::default()
        };

        Ok(store.save(&links_ipld).await?)
    }

    pub async fn for_each<ForEach>(&self, for_each: ForEach) -> Result<()>
    where
        ForEach: FnMut(&String, &Cid) -> Result<()>,
    {
        self.try_get_hamt().await?.for_each(for_each).await
    }

    pub async fn stream<'a>(
        &'a self,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<(&'a String, &'a Cid)>> + 'a>>> {
        Ok(self.try_get_hamt().await?.stream())
    }
}
