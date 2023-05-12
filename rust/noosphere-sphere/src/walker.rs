use anyhow::Result;
use cid::Cid;
use noosphere_core::data::{IdentityIpld, MapOperation};
use std::{collections::BTreeSet, marker::PhantomData};

use async_stream::try_stream;
use noosphere_storage::Storage;
use tokio::io::AsyncRead;
use tokio_stream::{Stream, StreamExt};
use ucan::crypto::KeyMaterial;

use crate::{
    content::{SphereContentRead, SphereFile},
    internal::SphereContextInternal,
    HasSphereContext, SpherePetnameRead,
};

/// A [SphereWalker] makes it possible to convert anything that implements
/// [HasSphereContext] into an async [Stream] over sphere content, allowing
/// incremental iteration over both the breadth of content at any version, or
/// the depth of changes over a range of history.
pub struct SphereWalker<C, K, S>
where
    C: HasSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    has_sphere_context: C,
    key: PhantomData<K>,
    storage: PhantomData<S>,
}

impl<C, K, S> From<C> for SphereWalker<C, K, S>
where
    C: HasSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    fn from(has_sphere_context: C) -> Self {
        SphereWalker {
            has_sphere_context,
            key: Default::default(),
            storage: Default::default(),
        }
    }
}

impl<C, K, S> SphereWalker<C, K, S>
where
    C: SpherePetnameRead<K, S> + HasSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    // Get a stream that yields every petname in the namespace along with its
    /// corresponding [AddressIpld]. This is useful for iterating over sphere
    /// petnames incrementally without having to load the entire index into
    /// memory at once.
    pub fn petname_stream(&self) -> impl Stream<Item = Result<(String, IdentityIpld)>> + '_ {
        try_stream! {
            let sphere = self.has_sphere_context.to_sphere().await?;
            let petnames = sphere.get_address_book().await?.get_identities().await?;
            let stream = petnames.into_stream().await?;

            for await entry in stream {
                let (petname, address) = entry?;
                yield (petname, address);
            }
        }
    }

    /// Get a stream that yields the set of petnames that changed at each
    /// revision of the backing sphere, up to but excluding an optional `since`
    /// CID parameter. To stream the entire history, pass `None` as the
    /// parameter.
    pub fn petname_change_stream<'a>(
        &'a self,
        since: Option<&'a Cid>,
    ) -> impl Stream<Item = Result<(Cid, BTreeSet<String>)>> + 'a {
        try_stream! {
            let sphere = self.has_sphere_context.to_sphere().await?;
            let since = since.cloned();
            let stream = sphere.into_identities_changelog_stream(since.as_ref());

            for await change in stream {
                let (cid, changelog) = change?;
                let mut changed_petnames = BTreeSet::new();

                for operation in changelog.changes {
                    let petname = match operation {
                        MapOperation::Add { key, .. } => key,
                        MapOperation::Remove { key } => key,
                    };
                    changed_petnames.insert(petname);
                }

                yield (cid, changed_petnames);
            }
        }
    }

    /// Get a [BTreeSet] whose members are all the petnames that have addresses
    /// as of this version of the sphere. Note that the full space of names may
    /// be very large; for a more space-efficient approach, use
    /// [SphereWalker::petname_stream] to incrementally access all petnames in
    /// the sphere.
    ///
    /// This method is forgiving of missing or corrupted data, and will yield an
    /// incomplete set of names in the case that some or all names are not able
    /// to be accessed.
    pub async fn list_petnames(&self) -> Result<BTreeSet<String>> {
        let sphere_identity = self.has_sphere_context.identity().await?;
        let petname_stream = self.petname_stream();

        tokio::pin!(petname_stream);

        Ok(petname_stream
            .fold(BTreeSet::new(), |mut petnames, another_petname| {
                match another_petname {
                    Ok((petname, _)) => {
                        petnames.insert(petname);
                    }
                    Err(error) => {
                        warn!(
                            "Could not read a petname from {}: {}",
                            sphere_identity, error
                        )
                    }
                };
                petnames
            })
            .await)
    }

    /// Get a [BTreeSet] whose members are all the petnames whose values have
    /// changed at least once since the provided version of the sphere
    /// (exclusive of the provided version; use `None` to get all petnames
    /// changed since the beginning of the sphere's history).
    ///
    /// This method is forgiving of missing or corrupted history, and will yield
    /// an incomplete set of changes in the case that some or all changes are
    /// not able to be accessed.
    ///
    /// Note that this operation will scale in memory consumption and duration
    /// proportionally to the size of the sphere and the length of its history.
    /// For a more efficient method of accessing changes, consider using
    /// [SphereWalker::petname_change_stream] instead.
    pub async fn petname_changes(&self, since: Option<&Cid>) -> Result<BTreeSet<String>> {
        let sphere_identity = self.has_sphere_context.identity().await?;
        let change_stream = self.petname_change_stream(since);

        tokio::pin!(change_stream);

        Ok(change_stream
            .fold(BTreeSet::new(), |mut all, some| {
                match some {
                    Ok((_, mut changes)) => all.append(&mut changes),
                    Err(error) => warn!(
                        "Could not read some changes from {}: {}",
                        sphere_identity, error
                    ),
                };
                all
            })
            .await)
    }
}

impl<C, K, S> SphereWalker<C, K, S>
where
    C: SphereContentRead<K, S> + HasSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    /// Same as [SphereWalker::content_stream], but consumes the [SphereWalker].
    /// This is useful in cases where it would otherwise be necessary to borrow
    /// a reference to [SphereWalker] for a static lifetime.
    pub fn into_content_stream(
        self,
    ) -> impl Stream<Item = Result<(String, SphereFile<impl AsyncRead>)>> {
        try_stream! {
            let sphere = self.has_sphere_context.to_sphere().await?;
            let content = sphere.get_content().await?;
            let stream = content.into_stream().await?;

            for await entry in stream {
                let (key, memo_link) = entry?;
                let file = self.has_sphere_context.get_file(sphere.cid(), memo_link).await?;

                yield (key.clone(), file);
            }
        }
    }

    /// Get a stream that yields every slug in the namespace along with its
    /// corresponding [SphereFile]. This is useful for iterating over sphere
    /// content incrementally without having to load the entire index into
    /// memory at once.
    pub fn content_stream(
        &self,
    ) -> impl Stream<Item = Result<(String, SphereFile<impl AsyncRead>)>> + '_ {
        try_stream! {
            let sphere = self.has_sphere_context.to_sphere().await?;
            let links = sphere.get_content().await?;
            let stream = links.into_stream().await?;

            for await entry in stream {
                let (key, memo) = entry?;
                let file = self.has_sphere_context.get_file(sphere.cid(), memo).await?;

                yield (key.clone(), file);
            }
        }
    }

    /// Get a stream that yields the set of slugs that changed at each revision
    /// of the backing sphere, up to but excluding an optional CID. To stream
    /// the entire history, pass `None` as the parameter.
    pub fn content_change_stream<'a>(
        &'a self,
        since: Option<&'a Cid>,
    ) -> impl Stream<Item = Result<(Cid, BTreeSet<String>)>> + 'a {
        try_stream! {
            let sphere = self.has_sphere_context.to_sphere().await?;
            let since = since.cloned();
            let stream = sphere.into_content_changelog_stream(since.as_ref());

            for await change in stream {
                let (cid, changelog) = change?;
                let mut changed_slugs = BTreeSet::new();

                for operation in changelog.changes {
                    let slug = match operation {
                        MapOperation::Add { key, .. } => key,
                        MapOperation::Remove { key } => key,
                    };
                    changed_slugs.insert(slug);
                }

                yield (cid, changed_slugs);
            }
        }
    }

    /// Get a [BTreeSet] whose members are all the slugs that have values as of
    /// this version of the sphere. Note that the full space of slugs may be
    /// very large; for a more space-efficient approach, use
    /// [SphereWalker::content_stream] or [SphereWalker::into_content_stream] to
    /// incrementally access all slugs in the sphere.
    ///
    /// This method is forgiving of missing or corrupted data, and will yield an
    /// incomplete set of links in the case that some or all links are not able
    /// to be accessed.
    pub async fn list_slugs(&self) -> Result<BTreeSet<String>> {
        let sphere_identity = self.has_sphere_context.identity().await?;
        let link_stream = self.content_stream();

        tokio::pin!(link_stream);

        Ok(link_stream
            .fold(BTreeSet::new(), |mut links, another_link| {
                match another_link {
                    Ok((slug, _)) => {
                        links.insert(slug);
                    }
                    Err(error) => {
                        warn!("Could not read a link from {}: {}", sphere_identity, error)
                    }
                };
                links
            })
            .await)
    }

    /// Get a [BTreeSet] whose members are all the slugs whose values have
    /// changed at least once since the provided version of the sphere
    /// (exclusive of the provided version; use `None` to get all slugs changed
    /// since the beginning of the sphere's history).
    ///
    /// This method is forgiving of missing or corrupted history, and will yield
    /// an incomplete set of changes in the case that some or all changes are
    /// not able to be accessed.
    ///
    /// Note that this operation will scale in memory consumption and duration
    /// proportionally to the size of the sphere and the length of its history.
    /// For a more efficient method of accessing changes, consider using
    /// [SphereWalker::content_change_stream] instead.
    pub async fn content_changes(&self, since: Option<&Cid>) -> Result<BTreeSet<String>> {
        let sphere_identity = self.has_sphere_context.identity().await?;
        let change_stream = self.content_change_stream(since);

        tokio::pin!(change_stream);

        Ok(change_stream
            .fold(BTreeSet::new(), |mut all, some| {
                match some {
                    Ok((_, mut changes)) => all.append(&mut changes),
                    Err(error) => warn!(
                        "Could not read some changes from {}: {}",
                        sphere_identity, error
                    ),
                };
                all
            })
            .await)
    }
}

#[cfg(test)]
pub mod tests {
    use std::collections::BTreeSet;

    use noosphere_core::data::ContentType;
    use tokio::io::AsyncReadExt;
    use tokio_stream::StreamExt;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use super::SphereWalker;
    use crate::helpers::{simulated_sphere_context, SimulationAccess};
    use crate::{HasMutableSphereContext, SphereContentWrite, SphereCursor};

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_be_initialized_with_a_context_or_a_cursor() {
        let sphere_context = simulated_sphere_context(SimulationAccess::ReadWrite, None)
            .await
            .unwrap();
        let mut cursor = SphereCursor::latest(sphere_context.clone());

        let changes = vec![
            vec!["dogs", "birds"],
            vec!["cats", "dogs"],
            vec!["birds"],
            vec!["cows", "beetles"],
        ];

        for change in changes {
            for slug in change {
                cursor
                    .write(
                        slug,
                        &ContentType::Subtext.to_string(),
                        b"are cool".as_ref(),
                        None,
                    )
                    .await
                    .unwrap();
            }

            cursor.save(None).await.unwrap();
        }

        let walker_cursor = SphereWalker::from(cursor);
        let walker_context = SphereWalker::from(sphere_context);

        let slugs_cursor = walker_cursor.list_slugs().await.unwrap();
        let slugs_context = walker_context.list_slugs().await.unwrap();

        assert_eq!(slugs_cursor.len(), 5);
        assert_eq!(slugs_cursor, slugs_context);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_list_all_slugs_currently_in_a_sphere() {
        let sphere_context = simulated_sphere_context(SimulationAccess::ReadWrite, None)
            .await
            .unwrap();
        let mut cursor = SphereCursor::latest(sphere_context);

        let changes = vec![
            vec!["dogs", "birds"],
            vec!["cats", "dogs"],
            vec!["birds"],
            vec!["cows", "beetles"],
        ];

        for change in changes {
            for slug in change {
                cursor
                    .write(
                        slug,
                        &ContentType::Subtext.to_string(),
                        b"are cool".as_ref(),
                        None,
                    )
                    .await
                    .unwrap();
            }

            cursor.save(None).await.unwrap();
        }

        let walker = SphereWalker::from(cursor.clone());
        let slugs = walker.list_slugs().await.unwrap();

        assert_eq!(slugs.len(), 5);

        cursor.remove("dogs").await.unwrap();
        cursor.save(None).await.unwrap();

        let slugs = walker.list_slugs().await.unwrap();

        assert_eq!(slugs.len(), 4);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_stream_the_whole_index() {
        let sphere_context = simulated_sphere_context(SimulationAccess::ReadWrite, None)
            .await
            .unwrap();
        let mut cursor = SphereCursor::latest(sphere_context);

        let expected = BTreeSet::<(String, String)>::from([
            ("cats".into(), "Cats are awesome".into()),
            ("dogs".into(), "Dogs are pretty cool".into()),
            ("birds".into(), "Birds rights".into()),
            ("mice".into(), "Mice like cookies".into()),
        ]);

        for (slug, content) in &expected {
            cursor
                .write(
                    slug.as_str(),
                    &ContentType::Subtext.to_string(),
                    content.as_ref(),
                    None,
                )
                .await
                .unwrap();

            cursor.save(None).await.unwrap();
        }

        let mut actual = BTreeSet::new();
        let walker = SphereWalker::from(cursor);
        let stream = walker.content_stream();

        tokio::pin!(stream);

        while let Some(Ok((slug, mut file))) = stream.next().await {
            let mut contents = String::new();
            file.contents.read_to_string(&mut contents).await.unwrap();
            actual.insert((slug, contents));
        }

        assert_eq!(expected, actual);
    }
}
