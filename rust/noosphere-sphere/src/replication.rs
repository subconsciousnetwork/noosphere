use anyhow::Result;
use async_stream::try_stream;
use bytes::Bytes;
use cid::Cid;
use futures_util::sink::SinkExt;
use libipld_cbor::DagCborCodec;
use noosphere_car::{CarHeader, CarWriter};
use noosphere_core::{
    data::{ContentType, MemoIpld, VersionedMapKey, VersionedMapValue},
    view::{Sphere, VersionedMap},
};
use noosphere_storage::{BlockStore, BlockStoreTap};
use std::io::{Error as IoError, ErrorKind as IoErrorKind};
use tokio::sync::mpsc::{channel, error::TryRecvError};
use tokio_stream::{Stream, StreamExt};
use tokio_util::{
    io::{CopyToBytes, SinkWriter},
    sync::PollSender,
};

use crate::BodyChunkDecoder;

pub(crate) async fn walk_versioned_map<K, V, S>(versioned_map: VersionedMap<K, V, S>) -> Result<()>
where
    K: VersionedMapKey + 'static,
    V: VersionedMapValue + 'static,
    S: BlockStore + 'static,
{
    versioned_map.get_changelog().await?;
    let stream = versioned_map.into_stream().await?;
    tokio::pin!(stream);
    while let Some(_) = stream.try_next().await? {}
    Ok(())
}

pub fn block_stream<S>(
    store: S,
    memo_version: Cid,
) -> impl Stream<Item = Result<(Cid, Vec<u8>)>> + Send
where
    S: BlockStore + 'static,
{
    try_stream! {
        let (store, mut rx) = BlockStoreTap::new(store.clone(), 64);
        let memo = store.load::<DagCborCodec, MemoIpld>(&memo_version).await?;

        match memo.content_type() {
            Some(ContentType::Sphere) => {
                let sphere = Sphere::from_memo(&memo, &store)?;
                let authority = sphere.get_authority().await?;
                let names = sphere.get_names().await?;
                let links = sphere.get_links().await?;
                let delegations = authority.get_delegations().await?;
                let revocations = authority.get_revocations().await?;

                // Drop, so that their internal store is dropped, so that the
                // store's internal sender is dropped, so that the receiver doesn't
                // think there are outstanding senders after our tasks are finished:
                drop(sphere);
                drop(authority);
                drop(store);

                let names_task = tokio::spawn(walk_versioned_map(names));
                let links_task = tokio::spawn(walk_versioned_map(links));
                let delegations_task = tokio::spawn(walk_versioned_map(delegations));
                let revocations_task = tokio::spawn(walk_versioned_map(revocations));

                while let Some(block) = rx.recv().await {
                    trace!("Yielding {}", block.0);
                    yield block;
                }

                let (names_result, links_result, delegations_result, revocations_result) = tokio::join!(
                    names_task,
                    links_task,
                    delegations_task,
                    revocations_task
                );

                names_result??;
                links_result??;
                delegations_result??;
                revocations_result??;
            }
            Some(_) => {
                let stream = BodyChunkDecoder(&memo.body, &store).stream();

                drop(store);

                tokio::pin!(stream);

                'decode: while let Some(_) = stream.try_next().await? {
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

pub fn car_stream<S>(
    store: S,
    memo_version: Cid,
) -> impl Stream<Item = Result<Bytes, IoError>> + Send
where
    S: BlockStore + 'static,
{
    try_stream! {
        let (tx, mut rx) = channel::<Bytes>(16);
        let sink =
            PollSender::new(tx).sink_map_err(|error| {
                error!("Failed to send CAR frame: {}", error);
                IoError::from(IoErrorKind::BrokenPipe)
            });

        let mut car_buffer = SinkWriter::new(CopyToBytes::new(sink));
        let car_header = CarHeader::new_v1(vec![memo_version]);
        let mut car_writer = CarWriter::new(car_header, &mut car_buffer);

        let block_stream = block_stream(
            store,
            memo_version.clone(),
        );

        for await item in block_stream {
            let (cid, block) = item.map_err(|error| {
                error!("Failed to stream blocks: {}", error);
                IoError::from(IoErrorKind::BrokenPipe)
            })?;

            car_writer.write(cid, block).await.map_err(|error| {
                error!("Failed to write CAR frame: {}", error);
                IoError::from(IoErrorKind::BrokenPipe)
            })?;

            car_writer.flush().await.map_err(|error| {
                error!("Failed to flush CAR frames: {}", error);
                IoError::from(IoErrorKind::BrokenPipe)
            })?;

            loop {
                match rx.try_recv() {
                    Ok(block) => yield block,
                    _ => break,
                };
            }
       }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use libipld_cbor::DagCborCodec;
    use noosphere_car::CarReader;
    use noosphere_core::{
        data::{BodyChunkIpld, ContentType, MemoIpld},
        tracing::initialize_tracing,
        view::Sphere,
    };
    use noosphere_storage::{BlockStore, MemoryStore};
    use tokio_stream::StreamExt;
    use tokio_util::io::StreamReader;

    use crate::{
        block_stream, car_stream,
        helpers::{simulated_sphere_context, SimulationAccess},
        walk_versioned_map, BodyChunkDecoder, HasMutableSphereContext, HasSphereContext,
        SphereContentWrite, SpherePetnameWrite,
    };

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_stream_all_blocks_in_a_sphere_version() {
        initialize_tracing();
        let mut sphere_context = simulated_sphere_context(SimulationAccess::ReadWrite)
            .await
            .unwrap();

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
                        &ContentType::Subtext.to_string(),
                        format!("{} are cool", slug).as_bytes(),
                        None,
                    )
                    .await
                    .unwrap();
            }

            for petname in petname_change {
                sphere_context
                    .set_petname(petname, Some(format!("did:key:{}", petname).into()))
                    .await
                    .unwrap();
            }

            sphere_context.save(None).await.unwrap();
        }

        let final_version = sphere_context.version().await.unwrap();

        let mut other_store = MemoryStore::default();

        let mut received = BTreeSet::new();
        let stream = block_stream(
            sphere_context.sphere_context().await.unwrap().db().clone(),
            final_version,
        );

        tokio::pin!(stream);

        while let Some((cid, block)) = stream.try_next().await.unwrap() {
            assert!(!received.contains(&cid));
            received.insert(cid.clone());
            other_store.put_block(&cid, &block).await.unwrap();
        }

        let sphere = Sphere::at(&final_version, &other_store);

        let links = sphere.get_links().await.unwrap();
        let petnames = sphere.get_names().await.unwrap();

        for (content_change, petname_change) in changes.iter() {
            for slug in content_change {
                let _ = links
                    .get(&slug.to_string())
                    .await
                    .unwrap()
                    .cloned()
                    .unwrap();
            }

            for petname in petname_change {
                let _ = petnames.get(&petname.to_string()).await.unwrap();
            }
        }

        let authority = sphere.get_authority().await.unwrap();
        let delegations = authority.get_delegations().await.unwrap();
        let revocations = authority.get_revocations().await.unwrap();

        walk_versioned_map(delegations).await.unwrap();
        walk_versioned_map(revocations).await.unwrap();
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_stream_all_blocks_in_some_sphere_content() {
        initialize_tracing();

        let mut sphere_context = simulated_sphere_context(SimulationAccess::ReadWrite)
            .await
            .unwrap();
        let mut db = sphere_context
            .sphere_context()
            .await
            .unwrap()
            .db_mut()
            .clone();

        let chunks = vec![b"foo", b"bar", b"baz"];

        let mut next_chunk_cid = None;

        for bytes in chunks.iter().rev() {
            next_chunk_cid = Some(
                db.save::<DagCborCodec, _>(&BodyChunkIpld {
                    bytes: bytes.to_vec(),
                    next: next_chunk_cid,
                })
                .await
                .unwrap(),
            );
        }

        let content_cid = sphere_context
            .link(
                "foo",
                &ContentType::Bytes.to_string(),
                &next_chunk_cid.unwrap(),
                None,
            )
            .await
            .unwrap();

        let stream = block_stream(
            sphere_context.sphere_context().await.unwrap().db().clone(),
            content_cid.clone(),
        );

        let mut store = MemoryStore::default();

        tokio::pin!(stream);

        while let Some((cid, block)) = stream.try_next().await.unwrap() {
            store.put_block(&cid, &block).await.unwrap();
        }

        let memo = store
            .load::<DagCborCodec, MemoIpld>(&content_cid)
            .await
            .unwrap();

        let mut buffer = Vec::new();
        let body_stream = BodyChunkDecoder(&memo.body, &store).stream();

        tokio::pin!(body_stream);

        while let Some(bytes) = body_stream.try_next().await.unwrap() {
            buffer.append(&mut Vec::from(bytes));
        }

        assert_eq!(buffer.as_slice(), b"foobarbaz");
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_stream_all_blocks_in_a_sphere_version_as_a_car() {
        initialize_tracing();

        let mut sphere_context = simulated_sphere_context(SimulationAccess::ReadWrite)
            .await
            .unwrap();

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
                        &ContentType::Subtext.to_string(),
                        format!("{} are cool", slug).as_bytes(),
                        None,
                    )
                    .await
                    .unwrap();
            }

            for petname in petname_change {
                sphere_context
                    .set_petname(petname, Some(format!("did:key:{}", petname).into()))
                    .await
                    .unwrap();
            }

            sphere_context.save(None).await.unwrap();
        }

        let final_version = sphere_context.version().await.unwrap();

        let mut other_store = MemoryStore::default();

        let stream = car_stream(
            sphere_context.sphere_context().await.unwrap().db().clone(),
            final_version,
        );

        tokio::pin!(stream);

        let reader = CarReader::new(StreamReader::new(stream)).await.unwrap();
        let block_stream = reader.stream();

        let mut received = BTreeSet::new();
        tokio::pin!(block_stream);

        while let Some((cid, block)) = block_stream.try_next().await.unwrap() {
            debug!("Received {cid}");
            assert!(!received.contains(&cid));
            received.insert(cid);
            other_store.put_block(&cid, &block).await.unwrap();
        }

        let sphere = Sphere::at(&final_version, &other_store);

        let links = sphere.get_links().await.unwrap();
        let petnames = sphere.get_names().await.unwrap();

        for (content_change, petname_change) in changes.iter() {
            for slug in content_change {
                let _ = links
                    .get(&slug.to_string())
                    .await
                    .unwrap()
                    .cloned()
                    .unwrap();
            }

            for petname in petname_change {
                let _ = petnames.get(&petname.to_string()).await.unwrap();
            }
        }

        let authority = sphere.get_authority().await.unwrap();
        let delegations = authority.get_delegations().await.unwrap();
        let revocations = authority.get_revocations().await.unwrap();

        walk_versioned_map(delegations).await.unwrap();
        walk_versioned_map(revocations).await.unwrap();
    }
}
