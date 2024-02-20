use std::str::FromStr;

use crate::{
    authority::collect_ucan_proofs,
    data::{ContentType, Link, MemoIpld, SphereIpld},
    view::Sphere,
};
use anyhow::{anyhow, Result};
use async_recursion::async_recursion;
use async_stream::try_stream;
use cid::Cid;
use libipld_cbor::DagCborCodec;
use noosphere_common::{spawn, ConditionalSend, TaskQueue};
use noosphere_storage::{BlockStore, BlockStoreTap, UcanStore};
use noosphere_ucan::{store::UcanJwtStore, Ucan};
use tokio::select;
use tokio_stream::{Stream, StreamExt};

use crate::stream::walk::{
    walk_versioned_map_changes_and, walk_versioned_map_elements, walk_versioned_map_elements_and,
};
use crate::view::BodyChunkDecoder;

/// Stream all the blocks required to reconstruct the history of a sphere since a
/// given point in time (or else the beginning of the history).
// TODO(tokio-rs/tracing#2503): instrument + impl trait causes clippy warning
#[allow(clippy::let_with_type_underscore)]
#[instrument(level = "trace", skip(store))]
pub fn memo_history_stream<S>(
    store: S,
    latest: &Link<MemoIpld>,
    since: Option<&Link<MemoIpld>>,
    include_content: bool,
) -> impl Stream<Item = Result<(Cid, Vec<u8>)>> + ConditionalSend
where
    S: BlockStore + 'static,
{
    debug!("Streaming history via memo...");

    let latest = *latest;
    let since = since.cloned();

    try_stream! {

        let (store, mut rx) = BlockStoreTap::new(store.clone(), 64);
        let memo = store.load::<DagCborCodec, MemoIpld>(&latest).await?;

        match memo.content_type() {
            Some(ContentType::Sphere) => {
                let mut history_task = Box::pin(spawn(async move {
                    let sphere = Sphere::from_memo(&memo, &store)?;
                    let identity = sphere.get_identity().await?;
                    let mut tasks = TaskQueue::default();

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
                            trace!("Replicating authority...");
                            let authority = sphere.get_authority().await?;
                            let store = store.clone();

                            tasks.spawn(async move {
                                let delegations = authority.get_delegations().await?;

                                walk_versioned_map_changes_and(delegations, store, |_, delegation, store| async move {
                                    let ucan_store = UcanStore(store);

                                    collect_ucan_proofs(&Ucan::from_str(&ucan_store.require_token(&delegation.jwt).await?)?, &ucan_store).await?;

                                    Ok(())
                                }).await?;

                                let revocations = authority.get_revocations().await?;
                                revocations.load_changelog().await?;

                                Ok(()) as Result<_, anyhow::Error>
                            });
                        }

                        if replicate_address_book {
                            trace!("Replicating address book...");
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
                            trace!("Replicating content...");
                            let content = sphere.get_content().await?;

                            tasks.spawn(walk_versioned_map_changes_and(content, store.clone(), move |_, link, store| async move {
                                if include_content {
                                    walk_memo_body(store, &link, include_content).await?;
                                } else {
                                    link.load_from(&store).await?;
                                };
                                Ok(())
                            }));
                        }

                        previous_sphere_body = Some(sphere_body);
                        previous_sphere_body_version = Some(sphere.to_memo().await?.body);

                        drop(sphere);
                    }

                    drop(store);

                    tasks.join().await?;

                    trace!("Done replicating!");

                    Ok(()) as Result<(), anyhow::Error>
                }));

                let mut receiver_is_open = true;
                let mut history_task_finished = false;
                let mut yield_count = 0usize;

                while receiver_is_open {
                    select! {
                        next = rx.recv() => {
                            if let Some(block) = next {
                                trace!(cid = ?block.0, "Yielding block {yield_count}...");
                                yield_count += 1;
                                yield block;
                            } else {
                                trace!("Receiver closed!");
                                receiver_is_open = false;
                            }
                            Ok(Ok::<_, anyhow::Error>(()))
                        },
                        history_result = &mut history_task, if !history_task_finished => {
                            trace!("History task completed!");
                            history_task_finished = true;
                            history_result
                        }
                    }??;
                }

                trace!("Done yielding {yield_count} blocks!");
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
    memo_version: &Link<MemoIpld>,
    include_content: bool,
) -> impl Stream<Item = Result<(Cid, Vec<u8>)>> + ConditionalSend
where
    S: BlockStore + 'static,
{
    debug!("Streaming body via memo...");

    let memo_version = *memo_version;

    try_stream! {
        let (store, mut rx) = BlockStoreTap::new(store.clone(), 1024);

        let mut receiver_is_open = true;
        let mut walk_memo_finished = false;
        let mut walk_memo_finishes = Box::pin(walk_memo_body(store, &memo_version, include_content));

        while receiver_is_open {
            select! {
                next = rx.recv() => {
                    if let Some(block) = next {
                        trace!("Yielding {}", block.0);
                        yield block;
                    } else {
                        receiver_is_open = false;
                    }
                    Ok::<_, anyhow::Error>(())
                },
                walk_memo_results = &mut walk_memo_finishes, if !walk_memo_finished => {
                    walk_memo_finished = true;
                    walk_memo_results
                }
            }?;
        }
    }
}

#[allow(clippy::let_with_type_underscore)]
#[instrument(level = "trace", skip(store))]
#[cfg_attr(target_arch="wasm32", async_recursion(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_recursion)]
async fn walk_memo_body<S>(
    store: S,
    memo_version: &Link<MemoIpld>,
    include_content: bool,
) -> Result<()>
where
    S: BlockStore + 'static,
{
    let memo = store.load::<DagCborCodec, MemoIpld>(memo_version).await?;
    match memo.content_type() {
        Some(ContentType::Sphere) => {
            let sphere = Sphere::from_memo(&memo, &store)?;
            let authority = sphere.get_authority().await?;
            let address_book = sphere.get_address_book().await?;
            let content = sphere.get_content().await?;
            let identities = address_book.get_identities().await?;
            let delegations = authority.get_delegations().await?;
            let revocations = authority.get_revocations().await?;

            let mut tasks = TaskQueue::default();

            tasks.spawn(walk_versioned_map_elements_and(
                identities,
                store.clone(),
                |_, identity, store| async move {
                    let ucan_store = UcanStore(store);
                    if let Some(link_record) = identity.link_record(&ucan_store).await {
                        link_record.collect_proofs(&ucan_store).await?;
                    };
                    Ok(())
                },
            ));

            tasks.spawn(walk_versioned_map_elements_and(
                content,
                store.clone(),
                move |_, link, store| async move {
                    if include_content {
                        walk_memo_body(store, &link, true).await?;
                    } else {
                        link.load_from(&store).await?;
                    }

                    Ok(())
                },
            ));

            tasks.spawn(async move {
                walk_versioned_map_elements_and(
                    delegations,
                    store,
                    |_, delegation, store| async move {
                        let ucan_store = UcanStore(store);

                        collect_ucan_proofs(
                            &Ucan::from_str(&ucan_store.require_token(&delegation.jwt).await?)?,
                            &ucan_store,
                        )
                        .await?;

                        Ok(())
                    },
                )
                .await?;

                Ok(()) as Result<_, anyhow::Error>
            });

            tasks.spawn(walk_versioned_map_elements(revocations));

            tasks.join().await?;
        }
        Some(_) => {
            let stream = BodyChunkDecoder(&memo.body, &store).stream();

            tokio::pin!(stream);

            while (stream.try_next().await?).is_some() {}
        }
        None => (),
    };

    Ok(())
}
