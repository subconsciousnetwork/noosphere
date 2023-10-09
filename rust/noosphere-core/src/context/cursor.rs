use crate::api::v0alpha1::ReplicateParameters;
use crate::stream::put_block_stream;
use crate::{
    data::{Link, MemoIpld},
    view::{Sphere, Timeline},
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use noosphere_common::StreamLatencyGuard;
use noosphere_storage::Storage;
use tokio::select;

use crate::context::{HasMutableSphereContext, HasSphereContext, SphereContext, SphereReplicaRead};
use instant::Duration;
use std::marker::PhantomData;

/// A [SphereCursor] is a structure that enables reading from and writing to a
/// [SphereContext] at specific versions of the associated sphere's history.
/// There are times when you may wish to be able to use the convenience
/// implementation of traits built on [HasSphereContext], but to always be sure
/// of what version you are using them on (such as when traversing sphere
/// history). That is when you would use a [SphereCursor], which can wrap any
/// implementor of [HasSphereContext] and mount it to a specific version of the
/// sphere.
#[derive(Clone)]
pub struct SphereCursor<C, S>
where
    C: HasSphereContext<S>,
    S: Storage + 'static,
{
    has_sphere_context: C,
    storage: PhantomData<S>,
    sphere_version: Option<Link<MemoIpld>>,
}

impl<C, S> SphereCursor<C, S>
where
    C: HasSphereContext<S>,
    S: Storage + 'static,
{
    /// Consume the [SphereCursor] and return its wrapped [HasSphereContext]
    pub fn to_inner(self) -> C {
        self.has_sphere_context
    }

    /// Same as [SphereCursor::mount], but mounts the [SphereCursor] to a known
    /// version of the history of the sphere.
    pub fn mounted_at(has_sphere_context: C, sphere_version: &Link<MemoIpld>) -> Self {
        SphereCursor {
            has_sphere_context,
            storage: PhantomData,
            sphere_version: Some(*sphere_version),
        }
    }

    /// Create the [SphereCursor] at the latest local version of the associated
    /// sphere, mounted to that version. If the latest version changes due to
    /// effects in the distance, the cursor will still point to the same version
    /// it referred to when it was created.
    pub async fn mounted(has_sphere_context: C) -> Result<Self> {
        let mut cursor = Self::latest(has_sphere_context);
        cursor.mount().await?;
        Ok(cursor)
    }

    /// "Mount" the [SphereCursor] to the given version of the sphere it refers
    /// to. If the [SphereCursor] is already mounted, the version it is mounted
    /// to will be overwritten. A mounted [SphereCursor] will remain at the
    /// version it is mounted to even when the latest version of the sphere
    /// changes.
    pub async fn mount_at(&mut self, sphere_version: &Link<MemoIpld>) -> Result<&Self> {
        self.sphere_version = Some(*sphere_version);

        Ok(self)
    }

    /// Same as [SphereCursor::mount_at] except that it mounts to the latest
    /// local version of the sphere.
    pub async fn mount(&mut self) -> Result<&Self> {
        let sphere_version = self
            .has_sphere_context
            .sphere_context()
            .await?
            .version()
            .await?;

        self.mount_at(&sphere_version).await
    }

    /// "Unmount" the [SphereCursor] so that it always uses the latest local
    /// version of the sphere that it refers to.
    pub fn unmount(mut self) -> Result<Self> {
        self.sphere_version = None;
        Ok(self)
    }

    /// Create this [SphereCursor] at the latest local version of the associated
    /// sphere. The [SphereCursor] will always point to the latest local
    /// version, unless subsequently mounted.
    pub fn latest(has_sphere_context: C) -> Self {
        SphereCursor {
            has_sphere_context,
            storage: PhantomData,
            sphere_version: None,
        }
    }

    /// Rewind the [SphereCursor] to point to the version of the sphere just
    /// prior to this one in the edit chronology. If there was a previous
    /// version to rewind to then the returned `Option` has the [Cid] of the
    /// revision, otherwise if the current version is the oldest one it is
    /// `None`.
    pub async fn rewind(&mut self) -> Result<Option<&Link<MemoIpld>>> {
        let sphere = self.to_sphere().await?;

        match sphere.get_parent().await? {
            Some(parent) => {
                self.sphere_version = Some(*parent.cid());
                Ok(self.sphere_version.as_ref())
            }
            None => Ok(None),
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<C, S> HasMutableSphereContext<S> for SphereCursor<C, S>
where
    C: HasMutableSphereContext<S>,
    S: Storage,
{
    type MutableSphereContext = C::MutableSphereContext;

    async fn sphere_context_mut(&mut self) -> Result<Self::MutableSphereContext> {
        self.has_sphere_context.sphere_context_mut().await
    }

    async fn save(
        &mut self,
        additional_headers: Option<Vec<(String, String)>>,
    ) -> Result<Link<MemoIpld>> {
        let new_version = self.has_sphere_context.save(additional_headers).await?;

        if self.sphere_version.is_some() {
            self.sphere_version = Some(new_version);
        }

        Ok(new_version)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<C, S> HasSphereContext<S> for SphereCursor<C, S>
where
    C: HasSphereContext<S>,
    S: Storage + 'static,
{
    type SphereContext = C::SphereContext;

    async fn sphere_context(&self) -> Result<Self::SphereContext> {
        self.has_sphere_context.sphere_context().await
    }

    async fn version(&self) -> Result<Link<MemoIpld>> {
        match &self.sphere_version {
            Some(sphere_version) => Ok(*sphere_version),
            None => self.has_sphere_context.version().await,
        }
    }

    async fn wrap(sphere_context: SphereContext<S>) -> Self {
        SphereCursor::latest(C::wrap(sphere_context).await)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<C, S> SphereReplicaRead<S> for SphereCursor<C, S>
where
    C: HasSphereContext<S>,
    S: Storage + 'static,
{
    #[instrument(level = "debug", skip(self))]
    async fn traverse_by_petnames(&self, petname_path: &[String]) -> Result<Option<Self>> {
        debug!("Traversing by petname...");

        let replicate = {
            let cursor = self.clone();

            move |version: Link<MemoIpld>, since: Option<Link<MemoIpld>>| {
                let cursor = cursor.clone();

                async move {
                    let replicate_parameters = since.as_ref().map(|since| ReplicateParameters {
                        since: Some(*since),
                    });
                    let (db, client) = {
                        let sphere_context = cursor.sphere_context().await?;
                        (sphere_context.db().clone(), sphere_context.client().await?)
                    };
                    let stream = client
                        .replicate(&version, replicate_parameters.as_ref())
                        .await?;

                    tokio::pin!(stream);

                    let (stream, mut rx) = StreamLatencyGuard::wrap(stream, Duration::from_secs(5));

                    select! {
                        _ = put_block_stream(db.clone(), stream) => (),
                        _ = rx.recv() => {
                            return Err(anyhow!("Block timed out"))
                        }
                    }

                    // If this was incremental replication, we have to hydrate...
                    if let Some(since) = since {
                        let since_memo = since.load_from(&db).await?;
                        let latest_memo = version.load_from(&db).await?;

                        // Only hydrate if since is a causal antecedent
                        if since_memo.lamport_order() < latest_memo.lamport_order() {
                            let timeline = Timeline::new(&db);

                            Sphere::hydrate_timeslice(
                                &timeline.slice(&version, Some(&since)).exclude_past(),
                            )
                            .await?;
                        }
                    }

                    Ok(()) as Result<(), anyhow::Error>
                }
            }
        };

        let sphere = self.to_sphere().await?;

        let peer_sphere = match sphere
            .traverse_by_petnames(petname_path, &replicate)
            .await?
        {
            Some(sphere) => sphere,
            None => return Ok(None),
        };

        let mut db = sphere.store().clone();
        let peer_identity = peer_sphere.get_identity().await?;
        let local_version = db.get_version(&peer_identity).await?.map(|cid| cid.into());

        let should_update_version = if let Some(since) = local_version {
            let since_memo = Sphere::at(&since, &db).to_memo().await?;
            let latest_memo = peer_sphere.to_memo().await?;

            since_memo.lamport_order() < latest_memo.lamport_order()
        } else {
            true
        };

        if should_update_version {
            debug!(
                "Updating local version of {} to more recent revision {}",
                peer_identity,
                peer_sphere.cid()
            );

            db.set_version(&peer_identity, peer_sphere.cid()).await?;
        }

        let peer_sphere_context = self
            .sphere_context()
            .await?
            .to_visitor(&peer_identity)
            .await?;

        Ok(Some(SphereCursor::mounted_at(
            C::wrap(peer_sphere_context).await,
            peer_sphere.cid(),
        )))
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use anyhow::Result;
    use noosphere_storage::{Store, UcanStore};
    use tokio::io::AsyncReadExt;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use crate::{
        authority::Access,
        context::{
            HasMutableSphereContext, HasSphereContext, SphereContentRead, SphereContentWrite,
            SpherePetnameRead, SpherePetnameWrite, SphereReplicaRead, SphereReplicaWrite,
        },
        data::{ContentType, Header},
        helpers::{
            make_sphere_context_with_peer_chain, make_valid_link_record, simulated_sphere_context,
        },
        tracing::initialize_tracing,
    };

    use super::SphereCursor;

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_unlink_slugs_from_the_content_space() {
        let (sphere_context, _) = simulated_sphere_context(Access::ReadWrite, None)
            .await
            .unwrap();
        let mut cursor = SphereCursor::latest(sphere_context);

        cursor
            .write(
                "cats",
                &ContentType::Subtext,
                b"Cats are great".as_ref(),
                None,
            )
            .await
            .unwrap();

        cursor.save(None).await.unwrap();

        assert!(cursor.read("cats").await.unwrap().is_some());

        cursor.remove("cats").await.unwrap();
        cursor.save(None).await.unwrap();

        assert!(cursor.read("cats").await.unwrap().is_none());
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_flushes_on_every_save() {
        let (sphere_context, _) = simulated_sphere_context(Access::ReadWrite, None)
            .await
            .unwrap();
        let initial_stats = {
            sphere_context
                .lock()
                .await
                .db()
                .to_block_store()
                .to_stats()
                .await
        };
        let mut cursor = SphereCursor::latest(sphere_context.clone());

        cursor
            .write(
                "cats",
                &ContentType::Subtext,
                b"Cats are great".as_ref(),
                None,
            )
            .await
            .unwrap();

        cursor.save(None).await.unwrap();

        let first_save_stats = {
            sphere_context
                .lock()
                .await
                .db()
                .to_block_store()
                .to_stats()
                .await
        };

        assert_eq!(first_save_stats.flushes, initial_stats.flushes + 1);

        cursor.remove("cats").await.unwrap();
        cursor.save(None).await.unwrap();

        let second_save_stats = {
            sphere_context
                .lock()
                .await
                .db()
                .to_block_store()
                .to_stats()
                .await
        };

        assert_eq!(second_save_stats.flushes, first_save_stats.flushes + 1);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_does_not_allow_writes_when_an_author_has_read_only_access() {
        let (sphere_context, _) = simulated_sphere_context(Access::ReadOnly, None)
            .await
            .unwrap();

        let mut cursor = SphereCursor::latest(sphere_context);

        let write_result = cursor
            .write(
                "cats",
                &ContentType::Subtext,
                b"Cats are great".as_ref(),
                None,
            )
            .await;

        assert!(write_result.is_err());
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_write_a_file_and_read_it_back() {
        let (sphere_context, _) = simulated_sphere_context(Access::ReadWrite, None)
            .await
            .unwrap();
        let mut cursor = SphereCursor::latest(sphere_context);

        cursor
            .write(
                "cats",
                &ContentType::Subtext,
                b"Cats are great".as_ref(),
                None,
            )
            .await
            .unwrap();

        cursor.save(None).await.unwrap();

        let mut file = cursor.read("cats").await.unwrap().unwrap();

        file.memo
            .expect_header(&Header::ContentType, &ContentType::Subtext)
            .unwrap();

        let mut value = String::new();
        file.contents.read_to_string(&mut value).await.unwrap();

        assert_eq!("Cats are great", value.as_str());
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_overwrite_a_file_with_new_contents_and_preserve_history() {
        let (sphere_context, _) = simulated_sphere_context(Access::ReadWrite, None)
            .await
            .unwrap();
        let mut cursor = SphereCursor::latest(sphere_context);

        cursor
            .write(
                "cats",
                &ContentType::Subtext,
                b"Cats are great".as_ref(),
                None,
            )
            .await
            .unwrap();

        cursor.save(None).await.unwrap();

        cursor
            .write(
                "cats",
                &ContentType::Subtext,
                b"Cats are better than dogs".as_ref(),
                None,
            )
            .await
            .unwrap();

        cursor.save(None).await.unwrap();

        let mut file = cursor.read("cats").await.unwrap().unwrap();

        file.memo
            .expect_header(&Header::ContentType, &ContentType::Subtext)
            .unwrap();

        let mut value = String::new();
        file.contents.read_to_string(&mut value).await.unwrap();

        assert_eq!("Cats are better than dogs", value.as_str());

        assert!(cursor.rewind().await.unwrap().is_some());

        file = cursor.read("cats").await.unwrap().unwrap();

        file.memo
            .expect_header(&Header::ContentType, &ContentType::Subtext)
            .unwrap();

        value.clear();
        file.contents.read_to_string(&mut value).await.unwrap();

        assert_eq!("Cats are great", value.as_str());
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_throws_an_error_when_saving_without_changes() {
        let (sphere_context, _) = simulated_sphere_context(Access::ReadWrite, None)
            .await
            .unwrap();
        let mut cursor = SphereCursor::latest(sphere_context);

        let result = cursor.save(None).await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "No changes to save");
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_throws_an_error_when_saving_with_empty_mutation_and_empty_headers() {
        let (sphere_context, _) = simulated_sphere_context(Access::ReadWrite, None)
            .await
            .unwrap();
        let mut cursor = SphereCursor::latest(sphere_context);

        let result = cursor.save(Some(vec![])).await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "No changes to save");
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_get_all_petnames_assigned_to_an_identity() -> Result<()> {
        let (sphere_context, _) = simulated_sphere_context(Access::ReadWrite, None).await?;

        let mut db = UcanStore(sphere_context.sphere_context().await?.db().clone());

        let (peer_1, link_record_1, _) = make_valid_link_record(&mut db).await?;
        let (peer_2, link_record_2, _) = make_valid_link_record(&mut db).await?;
        let (peer_3, link_record_3, _) = make_valid_link_record(&mut db).await?;

        let mut cursor = SphereCursor::latest(sphere_context);

        cursor
            .set_petname("foo1", Some(link_record_1.to_sphere_identity()))
            .await?;
        cursor
            .set_petname("bar1", Some(link_record_1.to_sphere_identity()))
            .await?;
        cursor
            .set_petname("baz1", Some(link_record_1.to_sphere_identity()))
            .await?;

        cursor
            .set_petname("foo2", Some(link_record_2.to_sphere_identity()))
            .await?;
        cursor.save(None).await?;

        cursor.set_petname_record("foo1", &link_record_1).await?;
        cursor.set_petname_record("bar1", &link_record_1).await?;
        cursor.set_petname_record("baz1", &link_record_1).await?;

        cursor.set_petname_record("foo2", &link_record_2).await?;

        cursor.save(None).await?;

        assert_eq!(
            cursor.get_assigned_petnames(&peer_1).await?,
            vec![
                String::from("foo1"),
                String::from("bar1"),
                String::from("baz1")
            ]
        );

        assert_eq!(
            cursor.get_assigned_petnames(&peer_2).await?,
            vec![String::from("foo2")]
        );

        assert_eq!(
            cursor.get_assigned_petnames(&peer_3).await?,
            Vec::<String>::new()
        );

        // Check one more time for good measure, since results are cached internally
        assert_eq!(
            cursor.get_assigned_petnames(&peer_1).await?,
            vec![
                String::from("foo1"),
                String::from("bar1"),
                String::from("baz1")
            ]
        );

        cursor
            .set_petname("bar2", Some(link_record_2.to_sphere_identity()))
            .await?;
        cursor
            .set_petname("foo3", Some(link_record_3.to_sphere_identity()))
            .await?;
        cursor.save(None).await?;

        cursor.set_petname_record("bar2", &link_record_2).await?;
        cursor.set_petname_record("foo3", &link_record_3).await?;
        cursor.save(None).await?;

        assert_eq!(
            cursor.get_assigned_petnames(&peer_1).await?,
            vec![
                String::from("foo1"),
                String::from("bar1"),
                String::from("baz1")
            ]
        );

        assert_eq!(
            cursor.get_assigned_petnames(&peer_2).await?,
            vec![String::from("bar2"), String::from("foo2")]
        );

        assert_eq!(
            cursor.get_assigned_petnames(&peer_3).await?,
            vec![String::from("foo3")]
        );

        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_traverse_a_sequence_of_petnames() -> Result<()> {
        initialize_tracing(None);

        let name_seqeuence: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        let (origin_sphere_context, _) =
            make_sphere_context_with_peer_chain(&name_seqeuence).await?;

        let cursor = SphereCursor::latest(Arc::new(
            origin_sphere_context.sphere_context().await?.clone(),
        ));

        let target_sphere_context = cursor
            .traverse_by_petnames(&name_seqeuence.into_iter().rev().collect::<Vec<String>>())
            .await?
            .unwrap();

        let mut name = String::new();
        let mut file = target_sphere_context.read("my-name").await?.unwrap();
        file.contents.read_to_string(&mut name).await?;

        assert_eq!(name.as_str(), "c");
        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_falls_back_to_available_version_when_traversal_target_version_is_not_available(
    ) -> Result<()> {
        initialize_tracing(None);

        let name_sequence: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        let (mut origin_sphere_context, mut sphere_contexts) =
            make_sphere_context_with_peer_chain(&name_sequence).await?;

        let expected_last_sphere_context_version =
            sphere_contexts.last().unwrap().version().await?;

        let mut db = origin_sphere_context.sphere_context().await?.db().clone();
        let mut next_link_record = None;
        let mut next_peer_name: Option<&str> = None;
        let mut last_context_latest_version = None;

        for (context, name) in sphere_contexts.iter_mut().zip(name_sequence.iter()).rev() {
            let current_version = context.version().await?;
            context
                .write("revision", &ContentType::Text, b"foo".as_ref(), None)
                .await?;

            if let Some(peer_name) = next_peer_name {
                context
                    .set_petname_record(peer_name, &next_link_record.unwrap())
                    .await?;
            }

            let version = context.save(None).await?;
            let link_record = context
                .create_link_record(Some(Duration::from_secs(130)))
                .await?;

            if last_context_latest_version.is_none() {
                last_context_latest_version = Some(version);
            }

            // Force the tip of history back to an old version in order to
            // simulate the "needs to replicate" condition (noting that this
            // pointer automatically advances for a local save).
            db.set_version(&context.identity().await?, &current_version)
                .await?;

            next_peer_name = Some(name.as_str());
            next_link_record = Some(link_record);
        }

        origin_sphere_context
            .set_petname_record("a", &next_link_record.unwrap())
            .await?;
        origin_sphere_context.save(None).await?;

        // Remove the memo for this version so that it cannot be found, forcing
        // a replicaton attempt that will fail when we try to traverse to it
        // later
        origin_sphere_context
            .sphere_context_mut()
            .await?
            .db_mut()
            .to_block_store()
            .remove(&last_context_latest_version.unwrap().to_bytes())
            .await?;

        let cursor = SphereCursor::latest(origin_sphere_context);

        let target_sphere_context = cursor
            .traverse_by_petnames(&name_sequence.into_iter().rev().collect::<Vec<String>>())
            .await?
            .unwrap();

        assert_eq!(
            target_sphere_context.version().await?,
            expected_last_sphere_context_version
        );

        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_resolves_none_when_a_petname_is_missing_from_the_sequence() -> Result<()> {
        initialize_tracing(None);

        let name_sequence: Vec<String> = vec!["b".into(), "c".into()];
        let (origin_sphere_context, _) =
            make_sphere_context_with_peer_chain(&name_sequence).await?;

        let cursor = SphereCursor::latest(Arc::new(
            origin_sphere_context.sphere_context().await?.clone(),
        ));

        let traversed_sequence: Vec<String> = vec!["a".into(), "b".into(), "c".into()];

        let target_sphere_context = cursor
            .traverse_by_petnames(
                &traversed_sequence
                    .into_iter()
                    .rev()
                    .collect::<Vec<String>>(),
            )
            .await
            .unwrap();

        assert!(target_sphere_context.is_none());

        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_correctly_identifies_a_visited_peer() -> Result<()> {
        initialize_tracing(None);

        let name_sequence: Vec<String> = vec!["a".into(), "b".into(), "c".into()];

        let (origin_sphere_context, sphere_contexts) =
            make_sphere_context_with_peer_chain(&name_sequence).await?;

        let mut dids = vec![origin_sphere_context.identity().await?];
        for context in &sphere_contexts {
            dids.push(context.identity().await?);
        }

        let cursor = SphereCursor::latest(Arc::new(
            origin_sphere_context.sphere_context().await?.clone(),
        ));

        let mut target_sphere_context = cursor;
        let mut identities = vec![target_sphere_context.identity().await?];

        for name in name_sequence.iter() {
            target_sphere_context = target_sphere_context
                .traverse_by_petnames(&[name.clone()])
                .await?
                .unwrap();
            identities.push(target_sphere_context.identity().await?);
        }

        assert_eq!(identities, dids);

        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_traverse_a_sequence_of_petnames_one_at_a_time() -> Result<()> {
        initialize_tracing(None);

        let name_sequence: Vec<String> = vec!["a".into(), "b".into(), "c".into()];

        let (origin_sphere_context, _) =
            make_sphere_context_with_peer_chain(&name_sequence).await?;

        let cursor = SphereCursor::latest(Arc::new(
            origin_sphere_context.sphere_context().await?.clone(),
        ));

        let mut target_sphere_context = cursor;

        for name in name_sequence.iter() {
            target_sphere_context = target_sphere_context
                .traverse_by_petnames(&[name.clone()])
                .await?
                .unwrap();
        }

        let mut name = String::new();
        let mut file = target_sphere_context
            .read("my-name")
            .await
            .unwrap()
            .unwrap();
        file.contents.read_to_string(&mut name).await.unwrap();

        assert_eq!(name.as_str(), "c");

        Ok(())
    }
}
