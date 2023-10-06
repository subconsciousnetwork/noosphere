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
    use cid::Cid;
    use libipld_core::{codec::Codec, ipld::Ipld, raw::RawCodec};
    use std::collections::BTreeSet;
    use ucan::{crypto::KeyMaterial, store::UcanJwtStore};

    use crate::{
        authority::{generate_ed25519_key, Access},
        context::{
            HasMutableSphereContext, HasSphereContext, SphereAuthorityWrite, SphereContentRead,
            SphereContentWrite, SpherePetnameWrite,
        },
        data::{BodyChunkIpld, ContentType, Link, LinkRecord, MemoIpld},
        helpers::{
            make_valid_link_record, simulated_sphere_context, touch_all_sphere_blocks,
            SimulatedHasMutableSphereContext,
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

    pub const SCAFFOLD_CHANGES: &[(&[&str], &[&str])] = &[
        (&["dogs", "birds"], &["alice", "bob"]),
        (&["cats", "dogs"], &["gordon"]),
        (&["birds"], &["cdata"]),
        (&["cows", "beetles"], &["jordan", "ben"]),
    ];

    pub async fn scaffold_sphere_context_with_history(
    ) -> Result<(SimulatedHasMutableSphereContext, Vec<Link<MemoIpld>>)> {
        let (mut sphere_context, _) = simulated_sphere_context(Access::ReadWrite, None).await?;
        let mut versions = Vec::new();
        let store = sphere_context.sphere_context().await?.db().clone();

        for (content_change, petname_change) in SCAFFOLD_CHANGES.iter() {
            for slug in *content_change {
                sphere_context
                    .write(
                        slug,
                        &ContentType::Subtext,
                        format!("{} are cool", slug).as_bytes(),
                        None,
                    )
                    .await?;
            }

            for petname in *petname_change {
                let (id, record, _) = make_valid_link_record(&mut UcanStore(store.clone())).await?;
                sphere_context.set_petname(petname, Some(id)).await?;
                versions.push(sphere_context.save(None).await?);
                sphere_context.set_petname_record(petname, &record).await?;
            }

            versions.push(sphere_context.save(None).await?);
        }

        let additional_device_credential = generate_ed25519_key();
        let additional_device_did = additional_device_credential.get_did().await?.into();
        let additional_device_authorization = sphere_context
            .authorize("otherdevice", &additional_device_did)
            .await?;

        sphere_context.save(None).await?;

        sphere_context
            .revoke_authorization(&additional_device_authorization)
            .await?;

        sphere_context.save(None).await?;

        Ok((sphere_context, versions))
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_includes_all_link_records_and_proofs_from_the_address_book() -> Result<()> {
        initialize_tracing(None);

        let (mut sphere_context, _) = simulated_sphere_context(Access::ReadWrite, None).await?;
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
            false,
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

        let (sphere_context, _) = scaffold_sphere_context_with_history().await?;

        let final_version = sphere_context.version().await?;

        let mut other_store = MemoryStore::default();

        let mut received = BTreeSet::new();

        let stream = memo_body_stream(
            sphere_context.sphere_context().await?.db().clone(),
            &final_version,
            false,
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

        for (content_change, petname_change) in SCAFFOLD_CHANGES.iter() {
            for slug in *content_change {
                let _ = content.get(&slug.to_string()).await?.cloned().unwrap();
            }

            for petname in *petname_change {
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

        let (sphere_context, versions) = scaffold_sphere_context_with_history().await?;

        let original_store = sphere_context.sphere_context().await?.db().clone();

        let mut other_store = MemoryStore::default();

        let first_version = versions.first().unwrap();
        let stream = memo_body_stream(original_store.clone(), first_version, false);

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

        let (mut sphere_context, _) = simulated_sphere_context(Access::ReadWrite, None).await?;
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
            false,
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

        let (mut sphere_context, _) = scaffold_sphere_context_with_history().await?;

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
            vec![final_version.into()],
            memo_body_stream(db.clone(), &final_version, false),
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

        for (content_change, petname_change) in SCAFFOLD_CHANGES.iter() {
            for slug in *content_change {
                let _ = content.get(&slug.to_string()).await?.cloned().unwrap();
            }

            for petname in *petname_change {
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

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_only_omits_memo_parent_references_when_streaming_sphere_body_with_content(
    ) -> Result<()> {
        initialize_tracing(None);

        let (sphere_context, mut versions) = scaffold_sphere_context_with_history().await?;

        debug!(
            "Versions: {:#?}",
            versions
                .iter()
                .map(|cid| cid.to_string())
                .collect::<Vec<String>>()
        );

        let store = sphere_context.lock().await.db().clone();
        let last_version = versions.pop().unwrap();
        let last_version_parent = versions.pop().unwrap();

        let mut links_referenced = BTreeSet::new();
        let mut links_included = BTreeSet::new();

        // The root is referenced implicitly
        links_referenced.insert(*last_version);

        let stream = memo_body_stream(store.clone(), &last_version, true);

        tokio::pin!(stream);

        while let Some((cid, block)) = stream.try_next().await? {
            if cid == *last_version {
                // Verify that parent of root is what we expect...
                let memo = store.load::<DagCborCodec, MemoIpld>(&cid).await?;
                assert_eq!(memo.parent, Some(last_version_parent));

                let codec = DagCborCodec;
                let mut root_references = BTreeSet::new();
                codec.references::<Ipld, BTreeSet<Cid>>(&block, &mut root_references)?;

                assert!(root_references.contains(&last_version_parent));
            }

            links_included.insert(cid);

            match cid.codec() {
                codec if codec == u64::from(DagCborCodec) => {
                    let codec = DagCborCodec;
                    codec.references::<Ipld, BTreeSet<Cid>>(&block, &mut links_referenced)?;
                }
                codec if codec == u64::from(RawCodec) => {
                    let codec = DagCborCodec;
                    codec.references::<Ipld, BTreeSet<Cid>>(&block, &mut links_referenced)?;
                }
                _ => {
                    unreachable!("No other codecs are used in our DAGs");
                }
            }
        }

        assert!(
            !links_included.contains(&last_version_parent),
            "Parent version should not be included"
        );

        let difference = links_referenced
            .difference(&links_included)
            .collect::<Vec<&Cid>>();

        debug!(
            "Difference: {:#?}",
            difference
                .iter()
                .map(|cid| cid.to_string())
                .collect::<Vec<String>>()
        );

        // These files have been each updated once after the first write, so their memos have
        // parent pointers to old versions that won't be included in the CAR
        let last_dogs_version = sphere_context
            .read("dogs")
            .await?
            .unwrap()
            .memo
            .parent
            .unwrap();
        let last_birds_version = sphere_context
            .read("birds")
            .await?
            .unwrap()
            .memo
            .parent
            .unwrap();

        let expected_difference: Vec<&Cid> = vec![
            &last_version_parent,
            &last_birds_version,
            &last_dogs_version,
        ];

        assert_eq!(difference.len(), expected_difference.len());

        for cid in expected_difference {
            assert!(difference.contains(&cid));
        }

        Ok(())
    }
}
