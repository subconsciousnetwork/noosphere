//! Utilities to support producing streams of blocks, as well as converting
//! streams of blocks to and from CARv1-encoded byte streams

mod block;
mod car;
mod memo;
mod walk;

pub use block::*;
pub use car::*;
pub use memo::*;
pub use walk::*;

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use std::collections::BTreeSet;
    use ucan::store::UcanJwtStore;

    use crate::{
        context::{
            HasMutableSphereContext, HasSphereContext, SphereContentWrite, SpherePetnameWrite,
        },
        data::{BodyChunkIpld, ContentType, LinkRecord, MemoIpld},
        helpers::{
            make_valid_link_record, simulated_sphere_context, touch_all_sphere_blocks,
            SimulationAccess,
        },
        stream::{from_car_stream, memo_body_stream, memo_history_stream, to_car_stream},
        tracing::initialize_tracing,
        view::{BodyChunkDecoder, Sphere},
    };
    use libipld_cbor::DagCborCodec;
    use noosphere_storage::{BlockStore, MemoryStore, UcanStore};
    use tokio_stream::StreamExt;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_includes_all_link_records_and_proofs_from_the_address_book() -> Result<()> {
        initialize_tracing(None);

        let (mut sphere_context, _) =
            simulated_sphere_context(SimulationAccess::ReadWrite, None).await?;
        let mut db = sphere_context.sphere_context().await?.db().clone();

        let (foo_did, foo_link_record, foo_link_record_link) =
            make_valid_link_record(&mut db).await?;

        sphere_context.set_petname("foo", Some(foo_did)).await?;
        sphere_context.save(None).await?;
        sphere_context
            .set_petname_record("foo", &foo_link_record)
            .await?;
        let final_version = sphere_context.save(None).await?;

        let mut other_store = MemoryStore::default();

        let stream = memo_body_stream(
            sphere_context.sphere_context().await?.db().clone(),
            &final_version,
        );

        tokio::pin!(stream);

        while let Some((cid, block)) = stream.try_next().await? {
            debug!("Received {cid}");
            other_store.put_block(&cid, &block).await?;
        }

        let ucan_store = UcanStore(other_store);

        let link_record =
            LinkRecord::try_from(ucan_store.require_token(&foo_link_record_link).await?)?;

        assert_eq!(link_record, foo_link_record);

        link_record.collect_proofs(&ucan_store).await?;

        Ok(())
    }

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
            false,
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

        let chunks = [b"foo", b"bar", b"baz"];

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

        let stream = to_car_stream(
            vec![final_version.clone().into()],
            memo_body_stream(db.clone(), &final_version),
        );

        let block_stream = from_car_stream(stream);

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
