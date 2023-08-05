use std::str::FromStr;

use anyhow::{anyhow, Result};
use async_stream::try_stream;
use cid::Cid;
use libipld_cbor::DagCborCodec;
use noosphere_core::{
    data::{ContentType, Link, LinkRecord, MemoIpld, SphereIpld},
    view::Sphere,
};
use noosphere_storage::{BlockStore, BlockStoreTap, UcanStore};
use tokio::sync::mpsc::error::TryRecvError;
use tokio::task::JoinSet;
use tokio_stream::{Stream, StreamExt};
use ucan::store::UcanJwtStore;

use crate::{
    walk_versioned_map_changes_and, walk_versioned_map_elements, walk_versioned_map_elements_and,
    BodyChunkDecoder,
};

/// Stream all the blocks required to reconstruct the history of a sphere since a
/// given point in time (or else the beginning of the history).
// TODO(tokio-rs/tracing#2503): instrument + impl trait causes clippy warning
#[allow(clippy::let_with_type_underscore)]
#[instrument(level = "trace", skip(store))]
pub fn memo_history_stream<S>(
    store: S,
    latest: &Link<MemoIpld>,
    since: Option<&Link<MemoIpld>>,
) -> impl Stream<Item = Result<(Cid, Vec<u8>)>> + Send
where
    S: BlockStore + 'static,
{
    let latest = latest.clone();
    let since = since.cloned();

    try_stream! {

        let (store, mut rx) = BlockStoreTap::new(store.clone(), 64);
        let memo = store.load::<DagCborCodec, MemoIpld>(&latest).await?;

        match memo.content_type() {
            Some(ContentType::Sphere) => {
                let history_task = tokio::spawn(async move {
                    let sphere = Sphere::from_memo(&memo, &store)?;
                    let identity = sphere.get_identity().await?;
                    let mut tasks = JoinSet::new();

                    let mut previous_sphere_body_version = None;
                    let mut previous_sphere_body: Option<SphereIpld> = None;

                    let history_stream = sphere.into_history_stream(since.as_ref());

                    tokio::pin!(history_stream);

                    while let Some((version, sphere)) = history_stream.try_next().await? {
                        if let Some(previous_sphere_body_version) = previous_sphere_body_version {
                            let memo = sphere.to_memo().await?;
                            if memo.body == previous_sphere_body_version {
                                warn!("Skipping {version} delta for {identity}, no sphere changes detected...");
                                continue;
                            }
                        }

                        debug!("Replicating {version} delta for {identity}");

                        let sphere_body = sphere.to_body().await?;
                        let (replicate_authority, replicate_address_book, replicate_content) = {
                            if let Some(previous_sphere_body) = previous_sphere_body {
                                (previous_sphere_body.authority != sphere_body.authority,
                                previous_sphere_body.address_book != sphere_body.address_book,
                                previous_sphere_body.content != sphere_body.content)
                            } else {
                                (true, true, true)
                            }
                        };

                        if replicate_authority {
                            debug!("Replicating authority...");
                            let authority = sphere.get_authority().await?;
                            let store = store.clone();

                            tasks.spawn(async move {
                                let delegations = authority.get_delegations().await?;

                                walk_versioned_map_changes_and(delegations, store, |_, delegation, store| async move {
                                    let ucan_store = UcanStore(store);

                                    LinkRecord::from_str(&ucan_store.require_token(&delegation.jwt).await?)?.collect_proofs(&ucan_store).await?;
                                    Ok(())
                                }).await?;

                                let revocations = authority.get_revocations().await?;
                                revocations.load_changelog().await?;

                                Ok(()) as Result<_, anyhow::Error>
                            });
                        }

                        if replicate_address_book {
                            debug!("Replicating address book...");
                            let address_book = sphere.get_address_book().await?;
                            let identities = address_book.get_identities().await?;

                            tasks.spawn(walk_versioned_map_changes_and(identities, store.clone(), |name, identity, store| async move {
                                let ucan_store = UcanStore(store);
                                trace!("Replicating proofs for {}", name);
                                if let Some(link_record) = identity.link_record(&ucan_store).await {
                                    link_record.collect_proofs(&ucan_store).await?;
                                };

                                Ok(())
                            }));
                        }

                        if replicate_content {
                            debug!("Replicating content...");
                            let content = sphere.get_content().await?;

                            tasks.spawn(walk_versioned_map_changes_and(content, store.clone(), |_, link, store| async move {
                                link.load_from(&store).await?;
                                Ok(())
                            }));
                        }

                        previous_sphere_body = Some(sphere_body);
                        previous_sphere_body_version = Some(sphere.to_memo().await?.body);

                        drop(sphere);
                    }

                    drop(store);

                    while let Some(result) = tasks.join_next().await {
                        trace!("Replication branch completed, {} remaining...", tasks.len());
                        result??;
                    }

                    trace!("Done replicating!");

                    Ok(()) as Result<(), anyhow::Error>
                });

                let mut yield_count = 0usize;

                while let Some(block) = rx.recv().await {
                    yield_count += 1;
                    trace!(cid = ?block.0, "Yielding block {yield_count}...");
                    yield block;
                }

                trace!("Done yielding {yield_count} blocks!");

                history_task.await??;
            }
            _ => {
                Err(anyhow!("History streaming is only supported for spheres, but {latest} has content type {:?})", memo.content_type()))?;
            }
        }
    }
}

/// Stream all the blocks required to read the sphere at a given version (making no
/// assumptions of what historical data may already be available to the reader).
// TODO(tokio-rs/tracing#2503): instrument + impl trait causes clippy warning
#[allow(clippy::let_with_type_underscore)]
#[instrument(level = "trace", skip(store))]
pub fn memo_body_stream<S>(
    store: S,
    memo_version: &Cid,
) -> impl Stream<Item = Result<(Cid, Vec<u8>)>> + Send
where
    S: BlockStore + 'static,
{
    let memo_version = *memo_version;

    try_stream! {
        let (store, mut rx) = BlockStoreTap::new(store.clone(), 1024);
        let memo = store.load::<DagCborCodec, MemoIpld>(&memo_version).await?;

        match memo.content_type() {
            Some(ContentType::Sphere) => {
                let sphere = Sphere::from_memo(&memo, &store)?;
                let authority = sphere.get_authority().await?;
                let address_book = sphere.get_address_book().await?;
                let content = sphere.get_content().await?;
                let identities = address_book.get_identities().await?;
                let delegations = authority.get_delegations().await?;
                let revocations = authority.get_revocations().await?;

                let identities_task = tokio::spawn(walk_versioned_map_elements_and(identities, store.clone(), |_, identity, store| async move {
                    let ucan_store = UcanStore(store);
                    if let Some(link_record) = identity.link_record(&ucan_store).await {
                        link_record.collect_proofs(&ucan_store).await?;
                    };
                    Ok(())
                }));
                let content_task = tokio::spawn(walk_versioned_map_elements_and(content, store.clone(), move |_, link, store| async move {
                    link.load_from(&store).await?;
                    Ok(())
                }));
                let delegations_task = tokio::spawn(walk_versioned_map_elements(delegations));
                let revocations_task = tokio::spawn(walk_versioned_map_elements(revocations));

                // Drop, so that their internal store is dropped, so that the
                // store's internal sender is dropped, so that the receiver doesn't
                // think there are outstanding senders after our tasks are finished:
                drop(sphere);
                drop(authority);
                drop(address_book);
                drop(store);

                while let Some(block) = rx.recv().await {
                    trace!("Yielding {}", block.0);
                    yield block;
                }

                let (identities_result, content_result, delegations_result, revocations_result) = tokio::join!(
                    identities_task,
                    content_task,
                    delegations_task,
                    revocations_task
                );

                identities_result??;
                content_result??;
                delegations_result??;
                revocations_result??;
            }
            Some(_) => {
                let stream = BodyChunkDecoder(&memo.body, &store).stream();

                drop(store);

                tokio::pin!(stream);

                'decode: while (stream.try_next().await?).is_some() {
                    'flush: loop {
                        match rx.try_recv() {
                            Ok(block) => {
                                yield block
                            },
                            Err(TryRecvError::Empty) => break 'flush,
                            Err(_) => break 'decode
                        };
                    }
                };
            }
            None => ()
        }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use std::collections::BTreeSet;

    use iroh_car::CarReader;
    use libipld_cbor::DagCborCodec;
    use noosphere_core::{
        data::{BodyChunkIpld, ContentType, MemoIpld},
        tracing::initialize_tracing,
        view::Sphere,
    };
    use noosphere_storage::{BlockStore, MemoryStore, UcanStore};
    use tokio_stream::StreamExt;
    use tokio_util::io::StreamReader;

    use crate::{
        car_stream,
        helpers::{
            make_valid_link_record, simulated_sphere_context, touch_all_sphere_blocks,
            SimulationAccess,
        },
        memo_body_stream, memo_history_stream, BodyChunkDecoder, HasMutableSphereContext,
        HasSphereContext, SphereContentWrite, SpherePetnameWrite,
    };

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_stream_all_blocks_in_a_sphere_version() -> Result<()> {
        initialize_tracing(None);

        let (mut sphere_context, _) =
            simulated_sphere_context(SimulationAccess::ReadWrite, None).await?;

        let changes = vec![
            (vec!["dogs", "birds"], vec!["alice", "bob"]),
            (vec!["cats", "dogs"], vec!["gordon"]),
            (vec!["birds"], vec!["cdata"]),
            (vec!["cows", "beetles"], vec!["jordan", "ben"]),
        ];

        for (content_change, petname_change) in changes.iter() {
            for slug in content_change {
                sphere_context
                    .write(
                        slug,
                        &ContentType::Subtext,
                        format!("{} are cool", slug).as_bytes(),
                        None,
                    )
                    .await?;
            }

            for petname in petname_change {
                sphere_context
                    .set_petname(petname, Some(format!("did:key:{}", petname).into()))
                    .await?;
            }

            sphere_context.save(None).await?;
        }

        let final_version = sphere_context.version().await?;

        let mut other_store = MemoryStore::default();

        let mut received = BTreeSet::new();

        let stream = memo_body_stream(
            sphere_context.sphere_context().await?.db().clone(),
            &final_version,
        );

        tokio::pin!(stream);

        while let Some((cid, block)) = stream.try_next().await? {
            debug!("Received {cid}");
            assert!(
                !received.contains(&cid),
                "Got {cid} but we already received it",
            );
            received.insert(cid);
            other_store.put_block(&cid, &block).await?;
        }

        let sphere = Sphere::at(&final_version, &other_store);

        let content = sphere.get_content().await?;
        let identities = sphere.get_address_book().await?.get_identities().await?;

        for (content_change, petname_change) in changes.iter() {
            for slug in content_change {
                let _ = content.get(&slug.to_string()).await?.cloned().unwrap();
            }

            for petname in petname_change {
                let _ = identities.get(&petname.to_string()).await?;
            }
        }

        touch_all_sphere_blocks(&sphere).await?;

        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_stream_all_delta_blocks_for_a_range_of_history() -> Result<()> {
        initialize_tracing(None);

        let (mut sphere_context, _) =
            simulated_sphere_context(SimulationAccess::ReadWrite, None).await?;

        let changes = vec![
            (vec!["dogs", "birds"], vec!["alice", "bob"]),
            (vec!["cats", "dogs"], vec!["gordon"]),
            (vec!["birds"], vec!["cdata"]),
            (vec!["cows", "beetles"], vec!["jordan", "ben"]),
        ];

        let original_store = sphere_context.sphere_context().await?.db().clone();
        let mut versions = Vec::new();

        for (content_change, petname_change) in changes.iter() {
            for slug in content_change {
                sphere_context
                    .write(
                        slug,
                        &ContentType::Subtext,
                        format!("{} are cool", slug).as_bytes(),
                        None,
                    )
                    .await?;
            }

            for petname in petname_change {
                let (id, record, _) =
                    make_valid_link_record(&mut UcanStore(original_store.clone())).await?;
                sphere_context.set_petname(petname, Some(id)).await?;
                versions.push(sphere_context.save(None).await?);
                sphere_context.set_petname_record(petname, &record).await?;
            }

            versions.push(sphere_context.save(None).await?);
        }

        let mut other_store = MemoryStore::default();

        let first_version = versions.first().unwrap();
        let stream = memo_body_stream(original_store.clone(), first_version);

        tokio::pin!(stream);

        while let Some((cid, block)) = stream.try_next().await? {
            other_store.put_block(&cid, &block).await?;
        }

        let sphere = Sphere::at(first_version, &other_store);

        touch_all_sphere_blocks(&sphere).await?;

        for i in 1..=3 {
            let version = versions.get(i).unwrap();
            let sphere = Sphere::at(version, &other_store);

            assert!(touch_all_sphere_blocks(&sphere).await.is_err());
        }

        let stream = memo_history_stream(
            original_store,
            versions.last().unwrap(),
            Some(first_version),
        );

        tokio::pin!(stream);

        while let Some((cid, block)) = stream.try_next().await? {
            other_store.put_block(&cid, &block).await?;
        }

        for i in 1..=3 {
            let version = versions.get(i).unwrap();
            let sphere = Sphere::at(version, &other_store);
            sphere.hydrate().await?;

            touch_all_sphere_blocks(&sphere).await?;
        }

        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_stream_all_blocks_in_some_sphere_content() -> Result<()> {
        initialize_tracing(None);

        let (mut sphere_context, _) =
            simulated_sphere_context(SimulationAccess::ReadWrite, None).await?;
        let mut db = sphere_context.sphere_context().await?.db_mut().clone();

        let chunks = vec![b"foo", b"bar", b"baz"];

        let mut next_chunk_cid = None;

        for bytes in chunks.iter().rev() {
            next_chunk_cid = Some(
                db.save::<DagCborCodec, _>(&BodyChunkIpld {
                    bytes: bytes.to_vec(),
                    next: next_chunk_cid,
                })
                .await?,
            );
        }

        let content_cid = sphere_context
            .link("foo", &ContentType::Bytes, &next_chunk_cid.unwrap(), None)
            .await?;

        let stream = memo_body_stream(
            sphere_context.sphere_context().await?.db().clone(),
            &content_cid,
        );

        let mut store = MemoryStore::default();

        tokio::pin!(stream);

        while let Some((cid, block)) = stream.try_next().await? {
            store.put_block(&cid, &block).await?;
        }

        let memo = store.load::<DagCborCodec, MemoIpld>(&content_cid).await?;

        let mut buffer = Vec::new();
        let body_stream = BodyChunkDecoder(&memo.body, &store).stream();

        tokio::pin!(body_stream);

        while let Some(bytes) = body_stream.try_next().await? {
            buffer.append(&mut Vec::from(bytes));
        }

        assert_eq!(buffer.as_slice(), b"foobarbaz");

        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_stream_all_blocks_in_a_sphere_version_as_a_car() -> Result<()> {
        initialize_tracing(None);

        let (mut sphere_context, _) =
            simulated_sphere_context(SimulationAccess::ReadWrite, None).await?;

        let changes = vec![
            (vec!["dogs", "birds"], vec!["alice", "bob"]),
            (vec!["cats", "dogs"], vec!["gordon"]),
            (vec!["birds"], vec!["cdata"]),
            (vec!["cows", "beetles"], vec!["jordan", "ben"]),
        ];

        for (content_change, petname_change) in changes.iter() {
            for slug in content_change {
                sphere_context
                    .write(
                        slug,
                        &ContentType::Subtext,
                        format!("{} are cool", slug).as_bytes(),
                        None,
                    )
                    .await?;
            }

            for petname in petname_change {
                sphere_context
                    .set_petname(petname, Some(format!("did:key:{}", petname).into()))
                    .await?;
            }

            sphere_context.save(None).await?;
        }

        let mut db = sphere_context.sphere_context().await?.db().clone();
        let (id, link_record, _) = make_valid_link_record(&mut db).await?;
        sphere_context.set_petname("hasrecord", Some(id)).await?;
        sphere_context.save(None).await?;
        sphere_context
            .set_petname_record("hasrecord", &link_record)
            .await?;
        sphere_context.save(None).await?;

        let final_version = sphere_context.version().await?;

        let mut other_store = MemoryStore::default();

        let stream = car_stream(
            vec![final_version.clone().into()],
            memo_body_stream(db.clone(), &final_version),
        );

        tokio::pin!(stream);

        let reader = CarReader::new(StreamReader::new(stream)).await?;
        let block_stream = reader.stream();

        let mut received = BTreeSet::new();
        tokio::pin!(block_stream);

        while let Some((cid, block)) = block_stream.try_next().await? {
            debug!("Received {cid}");
            assert!(
                !received.contains(&cid),
                "Got {cid} but we already received it",
            );
            received.insert(cid);
            other_store.put_block(&cid, &block).await?;
        }

        let sphere = Sphere::at(&final_version, &other_store);

        let content = sphere.get_content().await?;
        let identities = sphere.get_address_book().await?.get_identities().await?;

        for (content_change, petname_change) in changes.iter() {
            for slug in content_change {
                let _ = content.get(&slug.to_string()).await?.cloned().unwrap();
            }

            for petname in petname_change {
                let _ = identities.get(&petname.to_string()).await?;
            }
        }

        let has_record = identities.get(&"hasrecord".into()).await?.unwrap();
        let has_record_version = has_record.link_record(&UcanStore(other_store)).await;

        assert!(
            has_record_version.is_some(),
            "We got a resolved link record from the stream"
        );

        touch_all_sphere_blocks(&sphere).await?;

        Ok(())
    }
}
