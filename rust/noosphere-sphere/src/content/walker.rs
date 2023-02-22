use anyhow::Result;
use cid::Cid;
use noosphere_core::data::MapOperation;
use std::{collections::BTreeSet, marker::PhantomData};

use async_stream::try_stream;
use noosphere_storage::Storage;
use tokio::io::AsyncRead;
use tokio_stream::{Stream, StreamExt};
use ucan::crypto::KeyMaterial;

use crate::{internal::SphereContextInternal, HasSphereContext};

use super::{SphereContentRead, SphereFile};

/// A [SphereContentWalker] makes it possible to convert anything that
/// implements [HasSphereContext] into an async [Stream] over sphere content,
/// allowing incremental iteration over both the breadth of content at any
/// version, or the depth of changes over a range of history.
pub struct SphereContentWalker<H, K, S>
where
    H: HasSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    has_sphere_context: H,
    key: PhantomData<K>,
    storage: PhantomData<S>,
}

impl<H, K, S> From<H> for SphereContentWalker<H, K, S>
where
    H: HasSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    fn from(has_sphere_context: H) -> Self {
        SphereContentWalker {
            has_sphere_context,
            key: Default::default(),
            storage: Default::default(),
        }
    }
}

impl<H, K, S> SphereContentWalker<H, K, S>
where
    H: SphereContentRead<K, S> + HasSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    /// Same as `stream`, but consumes the [SphereFs]. This is useful in cases
    /// where it would otherwise be necessary to borrow a reference to
    /// [SphereFs] for a static lifetime.
    pub fn into_stream(self) -> impl Stream<Item = Result<(String, SphereFile<impl AsyncRead>)>> {
        try_stream! {
            let sphere = self.has_sphere_context.to_sphere().await?;
            let links = sphere.get_links().await?;
            let stream = links.stream().await?;

            for await entry in stream {
                let (key, memo_revision) = entry?;
                let file = self.has_sphere_context.get_file(sphere.cid(), memo_revision).await?;

                yield (key.clone(), file);
            }
        }
    }

    /// Get a stream that yields every slug in the namespace along with its
    /// corresponding [SphereFile]. This is useful for iterating over sphere
    /// content incrementally without having to load the entire index into
    /// memory at once.
    pub fn stream<'a>(
        &'a self,
    ) -> impl Stream<Item = Result<(String, SphereFile<impl AsyncRead + 'a>)>> {
        try_stream! {
            let sphere = self.has_sphere_context.to_sphere().await?;
            let links = sphere.get_links().await?;
            let stream = links.stream().await?;

            for await entry in stream {
                let (key, memo_revision) = entry?;
                let file = self.has_sphere_context.get_file(sphere.cid(), memo_revision).await?;

                yield (key.clone(), file);
            }
        }
    }

    /// Get a stream that yields the set of slugs that changed at each revision
    /// of the backing sphere, up to but excluding an optional CID. To stream
    /// the entire history, pass `None` as the parameter.
    pub fn change_stream<'a>(
        &'a self,
        since: Option<&'a Cid>,
    ) -> impl Stream<Item = Result<(Cid, BTreeSet<String>)>> + 'a {
        try_stream! {
            let sphere = self.has_sphere_context.to_sphere().await?;
            let since = since.cloned();
            let stream = sphere.into_link_changelog_stream(since.as_ref());

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
    /// very large; for a more space-efficient approach, use [SphereFs::stream]
    /// or [SphereFs::into_stream] to incrementally access all slugs in the
    /// sphere.
    ///
    /// This method is forgiving of missing or corrupted data, and will yield
    /// an incomplete set of links in the case that some or all links are
    /// not able to be accessed.
    pub async fn list(&self) -> Result<BTreeSet<String>> {
        let sphere_identity = self.has_sphere_context.identity().await?;
        let link_stream = self.stream();

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
    /// [SphereFs::change_stream] instead.
    pub async fn changes(&self, since: Option<&Cid>) -> Result<BTreeSet<String>> {
        let sphere_identity = self.has_sphere_context.identity().await?;
        let change_stream = self.change_stream(since);

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

    use crate::{
        helpers::{simulated_sphere_context, SimulationAccess},
        SphereContentWalker,
    };
    use crate::{HasMutableSphereContext, SphereContentWrite, SphereCursor};

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_be_initialized_with_a_context_or_a_cursor() {
        let sphere_context = simulated_sphere_context(SimulationAccess::ReadWrite)
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

        let walker_cursor = SphereContentWalker::from(cursor);
        let walker_context = SphereContentWalker::from(sphere_context);

        let slugs_cursor = walker_cursor.list().await.unwrap();
        let slugs_context = walker_context.list().await.unwrap();

        assert_eq!(slugs_cursor.len(), 5);
        assert_eq!(slugs_cursor, slugs_context);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_list_all_slugs_currently_in_a_sphere() {
        let sphere_context = simulated_sphere_context(SimulationAccess::ReadWrite)
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

        let walker = SphereContentWalker::from(cursor.clone());
        let slugs = walker.list().await.unwrap();

        assert_eq!(slugs.len(), 5);

        cursor.remove("dogs").await.unwrap();
        cursor.save(None).await.unwrap();

        let slugs = walker.list().await.unwrap();

        assert_eq!(slugs.len(), 4);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_stream_the_whole_index() {
        let sphere_context = simulated_sphere_context(SimulationAccess::ReadWrite)
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
        let walker = SphereContentWalker::from(cursor);
        let stream = walker.stream();

        tokio::pin!(stream);

        while let Some(Ok((slug, mut file))) = stream.next().await {
            let mut contents = String::new();
            file.contents.read_to_string(&mut contents).await.unwrap();
            actual.insert((slug, contents));
        }

        assert_eq!(expected, actual);
    }
}
