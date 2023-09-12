#![cfg(all(feature = "test-kubo", not(target_arch = "wasm32")))]

//! Integration tests that expect "full stack" Noosphere to be available, including
//! name system and block syndication backend (e.g., IPFS Kubo). The tests in this
//! module represent basic distributed system scenarios.

// TODO(#629): Remove this when we migrate off of `release-please`
extern crate noosphere_cli_dev as noosphere_cli;

#[macro_use]
extern crate tracing;

use anyhow::Result;
use noosphere_cli::helpers::{start_name_system_server, SpherePair};
use noosphere_common::helpers::wait;
use noosphere_core::{
    context::{
        HasMutableSphereContext, HasSphereContext, SphereContentRead, SphereContentWrite,
        SphereCursor, SpherePetnameRead, SpherePetnameWrite, SphereReplicaRead, SphereSync,
        SphereWalker, SyncRecovery,
    },
    data::{ContentType, Did, Link, MemoIpld},
    stream::memo_history_stream,
    tracing::initialize_tracing,
};
use noosphere_ns::{server::HttpClient, NameResolver};
use std::collections::BTreeSet;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio_stream::StreamExt;
use url::Url;

#[tokio::test]
async fn gateway_publishes_and_resolves_petnames_configured_by_the_client() -> Result<()> {
    initialize_tracing(None);

    let ipfs_url = Url::parse("http://127.0.0.1:5001")?;
    let (ns_url, ns_task) = start_name_system_server(&ipfs_url).await?;

    let mut base_pair = SpherePair::new("BASE", &ipfs_url, &ns_url).await?;
    let mut other_pair = SpherePair::new("OTHER", &ipfs_url, &ns_url).await?;

    base_pair.start_gateway().await?;
    other_pair.start_gateway().await?;

    let other_version = other_pair
        .spawn(|mut ctx| async move {
            ctx.write("foo", "text/plain", "bar".as_ref(), None).await?;
            let version = ctx.save(None).await?;
            ctx.sync(SyncRecovery::Retry(3)).await?;
            wait(1).await;
            Ok(version)
        })
        .await?;

    {
        let other_pair_identity = other_pair.client.identity.clone();
        let other_link = base_pair
            .spawn(|mut ctx| async move {
                ctx.set_petname("thirdparty", Some(other_pair_identity))
                    .await?;
                ctx.save(None).await?;
                ctx.sync(SyncRecovery::Retry(3)).await?;
                wait(1).await;

                ctx.sync(SyncRecovery::Retry(3)).await?;
                ctx.resolve_petname("thirdparty").await
            })
            .await?;
        assert_eq!(other_link, Some(other_version));
    }

    ns_task.abort();
    base_pair.stop_gateway().await?;
    other_pair.stop_gateway().await?;

    // Restart gateway and name system, ensuring republishing occurs
    let (ns_url, ns_task) = start_name_system_server(&ipfs_url).await?;
    let ns_client = HttpClient::new(ns_url.clone()).await?;
    assert!(
        ns_client
            .resolve(&base_pair.client.identity)
            .await?
            .is_none(),
        "new name system does not contain client identity"
    );

    base_pair.ns_url = ns_url.clone();
    base_pair.start_gateway().await?;
    wait(1).await;

    assert!(
        ns_client
            .resolve(&base_pair.client.identity)
            .await?
            .is_some(),
        "the gateway republishes records on start."
    );
    base_pair.stop_gateway().await?;
    ns_task.abort();
    Ok(())
}

/// Test that we can read from an adjacent, followed sphere, as well
/// as a followed sphere's followed sphere.
#[tokio::test]
async fn traverse_spheres_and_read_content_via_noosphere_gateway_via_ipfs() -> Result<()> {
    initialize_tracing(None);

    let ipfs_url = Url::parse("http://127.0.0.1:5001")?;
    let (ns_url, ns_task) = start_name_system_server(&ipfs_url).await?;

    let mut pair_1 = SpherePair::new("pair_1", &ipfs_url, &ns_url).await?;
    let mut pair_2 = SpherePair::new("pair_2", &ipfs_url, &ns_url).await?;
    let mut pair_3 = SpherePair::new("pair_3", &ipfs_url, &ns_url).await?;

    pair_1.start_gateway().await?;
    pair_2.start_gateway().await?;
    pair_3.start_gateway().await?;

    // Write some content in each sphere and track the versions after saving for later
    for pair in [&pair_1, &pair_2, &pair_3] {
        let name = pair.name.clone();
        let mut ctx = pair.sphere_context().await?;
        ctx.write("my-name", "text/plain", name.as_ref(), None)
            .await?;
        ctx.save(None).await?;
        ctx.sync(SyncRecovery::Retry(3)).await?;
    }
    wait(1).await;

    let id_2 = pair_2.client.identity.clone();
    let id_3 = pair_3.client.identity.clone();

    let pair_2_version = pair_2
        .spawn(|mut ctx| async move {
            ctx.set_petname("pair_3".into(), Some(id_3)).await?;
            ctx.save(None).await?;
            ctx.sync(SyncRecovery::Retry(3)).await?;
            wait(1).await;
            ctx.sync(SyncRecovery::Retry(3)).await?;
            assert!(ctx.resolve_petname("pair_3").await?.is_some());
            Ok(ctx.version().await?)
        })
        .await?;

    pair_1
        .spawn(move |mut ctx| async move {
            ctx.set_petname("pair_2".into(), Some(id_2)).await?;
            ctx.save(None).await?;
            ctx.sync(SyncRecovery::Retry(3)).await?;
            wait(1).await;
            ctx.sync(SyncRecovery::Retry(3)).await?;
            assert_eq!(ctx.resolve_petname("pair_2").await?, Some(pair_2_version));
            Ok(())
        })
        .await?;

    pair_1
        .spawn(|mut ctx| async move {
            ctx.sync(SyncRecovery::Retry(3)).await?;
            let cursor = SphereCursor::latest(Arc::new(ctx.sphere_context().await?.clone()));
            let pair_2_context = cursor
                .traverse_by_petnames(&["pair_2".to_string()])
                .await?
                .unwrap();

            debug!("Reading file from local third party sphere context...");
            let mut file = pair_2_context.read("my-name").await?.unwrap();
            let mut content = String::new();
            file.contents.read_to_string(&mut content).await?;
            assert_eq!(
                content.as_str(),
                "pair_2",
                "can read content from adjacent sphere"
            );

            // TODO(#320)
            let pair_3_context = pair_2_context
                .traverse_by_petnames(&["pair_3".to_string()])
                .await?
                .unwrap();

            debug!("Reading file from local leap-following third party sphere context...");

            let mut file = pair_3_context.read("my-name").await?.unwrap();
            let mut content = String::new();
            file.contents.read_to_string(&mut content).await.unwrap();
            assert_eq!(
                content.as_str(),
                "pair_3",
                "can read content from adjacent-adjacent sphere"
            );
            Ok(())
        })
        .await?;
    ns_task.abort();
    Ok(())
}

#[tokio::test]
async fn synchronize_petnames_as_they_are_added_and_removed() -> Result<()> {
    initialize_tracing(None);

    let ipfs_url = Url::parse("http://127.0.0.1:5001").unwrap();
    let (ns_url, ns_task) = start_name_system_server(&ipfs_url).await.unwrap();

    let mut base_pair = SpherePair::new("BASE", &ipfs_url, &ns_url).await?;
    let mut other_pair = SpherePair::new("OTHER", &ipfs_url, &ns_url).await?;
    let mut third_pair = SpherePair::new("THIRD", &ipfs_url, &ns_url).await?;

    base_pair.start_gateway().await?;
    other_pair.start_gateway().await?;
    third_pair.start_gateway().await?;

    let other_pair_id = other_pair.client.identity.clone();
    let other_version = other_pair
        .spawn(|mut ctx| async move {
            ctx.write("foo", "text/plain", "bar".as_ref(), None).await?;
            let version = ctx.save(None).await?;
            ctx.sync(SyncRecovery::Retry(3)).await?;
            wait(1).await;
            Ok(version)
        })
        .await?;

    let third_pair_id = third_pair.client.identity.clone();
    let third_version = third_pair
        .spawn(|mut ctx| async move {
            ctx.write("bar", "text/plain", "baz".as_ref(), None).await?;
            let version = ctx.save(None).await?;
            ctx.sync(SyncRecovery::Retry(3)).await?;
            wait(1).await;
            Ok(version)
        })
        .await?;

    base_pair
        .spawn(move |mut ctx| async move {
            ctx.set_petname("thirdparty", Some(other_pair_id)).await?;
            ctx.save(None).await?;
            ctx.sync(SyncRecovery::Retry(3)).await?;
            wait(1).await;

            ctx.sync(SyncRecovery::Retry(3)).await?;
            let other_link = ctx.resolve_petname("thirdparty").await?;
            assert_eq!(other_link, Some(other_version.clone()));

            let resolved = ctx.resolve_petname("thirdparty").await?;
            assert!(resolved.is_some());

            info!("UNSETTING 'thirdparty' as a petname and syncing again...");
            ctx.set_petname("thirdparty", None).await?;
            ctx.save(None).await?;
            ctx.sync(SyncRecovery::Retry(3)).await?;
            wait(1).await;
            ctx.sync(SyncRecovery::Retry(3)).await?;
            let resolved = ctx.resolve_petname("thirdparty").await?;
            assert!(resolved.is_none());
            let recorded = ctx.get_petname("thirdparty").await?;
            assert!(recorded.is_none());

            info!("SETTING 'thirdparty' petname to a different identity and syncing again...");
            ctx.set_petname("thirdparty", Some(third_pair_id.clone()))
                .await?;
            ctx.save(None).await?;
            ctx.sync(SyncRecovery::Retry(3)).await?;
            wait(1).await;
            ctx.sync(SyncRecovery::Retry(3)).await?;

            let saved_id = ctx.get_petname("thirdparty").await?;
            assert_eq!(saved_id, Some(third_pair_id));

            let third_link = ctx.resolve_petname("thirdparty").await?;
            assert_eq!(third_link, Some(third_version.clone()));

            Ok(())
        })
        .await?;

    ns_task.abort();
    Ok(())
}

#[tokio::test]
async fn traverse_spheres_and_get_incremental_updates_via_noosphere_gateway_via_ipfs() -> Result<()>
{
    initialize_tracing(None);

    let ipfs_url = Url::parse("http://127.0.0.1:5001")?;
    let (ns_url, ns_task) = start_name_system_server(&ipfs_url).await?;

    let mut pair_1 = SpherePair::new("pair_1", &ipfs_url, &ns_url).await?;
    let mut pair_2 = SpherePair::new("pair_2", &ipfs_url, &ns_url).await?;

    pair_1.start_gateway().await?;
    pair_2.start_gateway().await?;

    // Write some content in each sphere and track the versions after saving for later
    for pair in [&pair_1, &pair_2] {
        let name = pair.name.clone();
        let mut ctx = pair.sphere_context().await?;
        ctx.write("my-name", "text/plain", name.as_ref(), None)
            .await?;
        ctx.save(None).await?;
        ctx.sync(SyncRecovery::Retry(3)).await?;
    }
    wait(1).await;

    let id_2 = pair_2.client.identity.clone();
    let pair_2_version = pair_2.sphere_context().await?.version().await?;

    pair_1
        .spawn(move |mut ctx| async move {
            ctx.set_petname("pair_2".into(), Some(id_2)).await?;
            ctx.save(None).await?;
            ctx.sync(SyncRecovery::Retry(3)).await?;
            wait(1).await;
            ctx.sync(SyncRecovery::Retry(3)).await?;
            assert_eq!(ctx.resolve_petname("pair_2").await?, Some(pair_2_version));
            Ok(())
        })
        .await?;

    pair_1
        .spawn(|mut ctx| async move {
            wait(1).await;
            ctx.sync(SyncRecovery::Retry(3)).await?;
            let cursor = SphereCursor::latest(Arc::new(ctx.sphere_context().await?.clone()));
            let pair_2_context = cursor
                .traverse_by_petnames(&["pair_2".to_string()])
                .await?
                .unwrap();

            debug!("Reading file from local third party sphere context...");
            let mut file = pair_2_context.read("my-name").await?.unwrap();
            let mut content = String::new();
            file.contents.read_to_string(&mut content).await?;
            assert_eq!(
                content.as_str(),
                "pair_2",
                "can read content from adjacent sphere"
            );

            Ok(())
        })
        .await?;

    pair_2
        .spawn(|mut ctx| async move {
            ctx.write("foo", &ContentType::Text, "foo".as_bytes(), None)
                .await?;
            ctx.save(None).await?;

            ctx.write("bar", &ContentType::Text, "bar".as_bytes(), None)
                .await?;
            ctx.save(None).await?;

            ctx.write("baz", &ContentType::Text, "baz".as_bytes(), None)
                .await?;
            ctx.save(None).await?;

            ctx.remove("my-name").await?;
            ctx.save(None).await?;

            let latest_version = ctx.sync(SyncRecovery::Retry(3)).await?;
            info!("Expect version: {}", latest_version);

            wait(1).await;

            Ok(())
        })
        .await?;

    let pair_2_identity = pair_2.sphere_context().await?.identity().await?;

    pair_1
        .spawn(|mut ctx| async move {
            // Set and sync a new petname to "force" name resolution in the gateway
            ctx.set_petname("foo", Some(Did("did:key:foo".into())))
                .await?;
            ctx.save(None).await?;
            ctx.sync(SyncRecovery::Retry(3)).await?;
            wait(1).await;
            ctx.sync(SyncRecovery::Retry(3)).await?;

            let cursor = SphereCursor::latest(Arc::new(ctx.sphere_context().await?.clone()));
            let pair_2_context = cursor
                .traverse_by_petnames(&vec!["pair_2".into()])
                .await?
                .unwrap();

            // Verify the identity hasn't been messed up to catch regressions
            // https://github.com/subconsciousnetwork/subconscious/issues/675
            let identity = pair_2_context.identity().await?;
            assert_eq!(identity, pair_2_identity);

            let version = pair_2_context.version().await?;
            info!("Have version: {}", version);

            let mut file = pair_2_context.read("baz").await?.unwrap();
            let mut content = String::new();
            file.contents.read_to_string(&mut content).await?;
            assert_eq!(content.as_str(), "baz");

            Ok(())
        })
        .await?;

    ns_task.abort();
    Ok(())
}

#[tokio::test]
async fn replicate_older_version_of_peer_than_the_one_you_have() -> Result<()> {
    initialize_tracing(None);

    let ipfs_url = Url::parse("http://127.0.0.1:5001")?;
    let (ns_url, ns_task) = start_name_system_server(&ipfs_url).await?;

    let mut pair_1 = SpherePair::new("pair_1", &ipfs_url, &ns_url).await?;
    let mut pair_2 = SpherePair::new("pair_2", &ipfs_url, &ns_url).await?;
    let mut pair_3 = SpherePair::new("pair_3", &ipfs_url, &ns_url).await?;

    pair_1.start_gateway().await?;
    pair_2.start_gateway().await?;
    pair_3.start_gateway().await?;

    let id_3 = pair_3.client.identity.clone();

    pair_3
        .spawn(|mut ctx| async move {
            ctx.sync(SyncRecovery::Retry(3)).await?;
            Ok(())
        })
        .await?;

    // sphere_2 follows sphere_3
    pair_2
        .spawn(|mut ctx| async move {
            ctx.set_petname("pair_3".into(), Some(id_3)).await?;
            ctx.save(None).await?;
            ctx.sync(SyncRecovery::Retry(3)).await?;
            wait(1).await;
            ctx.sync(SyncRecovery::Retry(3)).await?;
            assert!(ctx.resolve_petname("pair_3").await?.is_some());
            Ok(ctx.version().await?)
        })
        .await?;

    let id_2 = pair_2.client.identity.clone();
    let id_3 = pair_3.client.identity.clone();

    // sphere_3 writes some initial content
    let sphere_3_first_version = pair_3
        .spawn(move |mut ctx| async move {
            ctx.write("foo", &ContentType::Text, "foo".as_bytes(), None)
                .await?;
            ctx.save(None).await?;
            let cid = ctx.sync(SyncRecovery::Retry(3)).await?;
            Ok(cid)
        })
        .await?;

    {
        let sphere_3_first_version = sphere_3_first_version.clone();
        // sphere_2 updates with sphere_3's initial content
        pair_2
            .spawn(move |mut ctx| async move {
                // Set and sync a new petname to "force" name resolution in the gateway
                ctx.set_petname("foo", Some(Did("did:key:foo".into())))
                    .await?;
                ctx.save(None).await?;
                ctx.sync(SyncRecovery::Retry(3)).await?;
                wait(1).await;
                ctx.sync(SyncRecovery::Retry(3)).await?;
                assert_eq!(
                    ctx.resolve_petname("pair_3").await?,
                    Some(sphere_3_first_version)
                );

                Ok(())
            })
            .await?;
    }
    // sphere_3 makes a bunch of additional changes
    let sphere_3_newest_version = pair_3
        .spawn(move |mut ctx| async move {
            ctx.write("foo", &ContentType::Text, "foo2".as_bytes(), None)
                .await?;
            ctx.save(None).await?;
            ctx.write("bar", &ContentType::Text, "bar".as_bytes(), None)
                .await?;
            ctx.save(None).await?;
            ctx.write("baz", &ContentType::Text, "baz".as_bytes(), None)
                .await?;
            ctx.save(None).await?;
            ctx.remove("bar").await?;
            ctx.save(None).await?;
            let cid = ctx.sync(SyncRecovery::Retry(3)).await?;
            Ok(cid)
        })
        .await?;

    // sphere_1 follows sphere_2 and sphere_3, then...
    // sphere_1 gets the latest version of sphere_3 and traverses to sphere_2's
    // sphere_3 (which is an older version than the oldest version sphere_1 has
    // seen)
    pair_1
        .spawn(move |mut ctx| async move {
            ctx.set_petname("pair_2".into(), Some(id_2)).await?;
            ctx.set_petname("pair_3".into(), Some(id_3)).await?;
            ctx.save(None).await?;
            ctx.sync(SyncRecovery::Retry(3)).await?;
            wait(1).await;
            ctx.sync(SyncRecovery::Retry(3)).await?;
            let cid = ctx.resolve_petname("pair_3").await?.unwrap();

            assert_eq!(cid, sphere_3_newest_version);

            let cursor = SphereCursor::latest(Arc::new(ctx.sphere_context().await?.clone()));
            let sphere_1_sphere_3_cursor = cursor
                .traverse_by_petnames(&["pair_3".to_string()])
                .await?
                .unwrap();

            // File we added
            let mut file = sphere_1_sphere_3_cursor.read("baz").await?.unwrap();
            let mut content = String::new();
            file.contents.read_to_string(&mut content).await?;

            assert_eq!(content, "baz");

            // File we removed
            let file = sphere_1_sphere_3_cursor.read("bar").await?;
            assert!(file.is_none());

            // File we changed
            let mut file = sphere_1_sphere_3_cursor.read("foo").await?.unwrap();
            let mut content = String::new();
            file.contents.read_to_string(&mut content).await?;

            assert_eq!(content, "foo2");

            let sphere_1_sphere_2_cursor = cursor
                .traverse_by_petnames(&["pair_2".to_string()])
                .await?
                .unwrap();

            assert_eq!(
                sphere_1_sphere_2_cursor.resolve_petname("pair_3").await?,
                Some(sphere_3_first_version)
            );

            let sphere_1_sphere_2_sphere_3_cursor = sphere_1_sphere_2_cursor
                .traverse_by_petnames(&["pair_3".into()])
                .await?
                .unwrap();

            let mut file = sphere_1_sphere_2_sphere_3_cursor
                .read("foo")
                .await?
                .unwrap();
            let mut content = String::new();
            file.contents.read_to_string(&mut content).await?;

            assert_eq!(content, "foo");

            Ok(())
        })
        .await?;

    ns_task.abort();
    Ok(())
}

#[tokio::test]
async fn local_lineage_remains_sparse_as_graph_changes_accrue_over_time() -> Result<()> {
    initialize_tracing(None);

    let ipfs_url = Url::parse("http://127.0.0.1:5001")?;
    let (ns_url, ns_task) = start_name_system_server(&ipfs_url).await?;

    let mut pair_1 = SpherePair::new("pair_1", &ipfs_url, &ns_url).await?;
    let mut pair_2 = SpherePair::new("pair_2", &ipfs_url, &ns_url).await?;

    pair_1.start_gateway().await?;
    pair_2.start_gateway().await?;

    pair_2
        .spawn(move |mut ctx| async move {
            ctx.write("peer-content", "text/plain", "baz".as_bytes(), None)
                .await?;

            ctx.save(None).await?;
            ctx.sync(SyncRecovery::Retry(3)).await?;
            Ok(())
        })
        .await?;

    let sphere_2_id = pair_2.client.identity.clone();

    pair_1
        .spawn(move |mut ctx| async move {
            ctx.write("some-content", "text/plain", "foobar".as_bytes(), None)
                .await?;

            ctx.save(None).await?;
            ctx.sync(SyncRecovery::Retry(3)).await?;

            ctx.write("new-content", "text/plain", "foobar2".as_bytes(), None)
                .await?;

            ctx.save(None).await?;
            ctx.sync(SyncRecovery::Retry(3)).await?;

            ctx.set_petname("my-peer", Some(sphere_2_id)).await?;
            ctx.save(None).await?;

            ctx.sync(SyncRecovery::Retry(3)).await?;
            wait(1).await;
            ctx.sync(SyncRecovery::Retry(3)).await?;

            Ok(())
        })
        .await?;

    pair_2
        .spawn(|mut ctx| async move {
            ctx.write("peer-content", "text/plain", "baz".as_bytes(), None)
                .await?;
            ctx.save(None).await?;
            ctx.sync(SyncRecovery::Retry(3)).await?;

            Ok(())
        })
        .await?;

    pair_1
        .spawn(|mut ctx| async move {
            ctx.sync(SyncRecovery::Retry(3)).await?;
            wait(1).await;
            ctx.sync(SyncRecovery::Retry(3)).await?;

            let walker = SphereWalker::from(&ctx);

            let content_history = walker.content_change_stream(None);
            tokio::pin!(content_history);

            let history = content_history
                .collect::<Result<Vec<(Link<MemoIpld>, BTreeSet<String>)>>>()
                .await?;

            for (cid, changes) in history.iter() {
                trace!("{}: {:?}", cid.to_string(), changes);
            }

            for (index, (version, content_changes)) in history.iter().enumerate() {
                debug!(history_index = ?index, version = ?version, changes = ?content_changes);
                match index {
                    0 => {
                        assert!(content_changes.contains(&"some-content".to_owned()));
                        assert_eq!(content_changes.len(), 1);
                    }
                    1 => {
                        assert!(content_changes.contains(&"new-content".to_owned()));
                        assert_eq!(content_changes.len(), 1);
                    }
                    _ => {
                        unreachable!("There should only be two revisions to content!")
                    }
                }
            }

            Ok(())
        })
        .await?;

    ns_task.abort();
    Ok(())
}

#[tokio::test]
async fn all_of_client_history_is_made_manifest_on_the_gateway_after_sync() -> Result<()> {
    initialize_tracing(None);

    let ipfs_url = Url::parse("http://127.0.0.1:5001")?;
    let (ns_url, ns_task) = start_name_system_server(&ipfs_url).await?;

    let mut pair_1 = SpherePair::new("ONE", &ipfs_url, &ns_url).await?;
    let mut pair_2 = SpherePair::new("TWO", &ipfs_url, &ns_url).await?;

    pair_1.start_gateway().await?;
    pair_2.start_gateway().await?;

    let _ = pair_2
        .spawn(|mut ctx| async move {
            ctx.write("foo", &ContentType::Text, "bar".as_bytes(), None)
                .await?;
            ctx.save(None).await?;
            Ok(ctx.sync(SyncRecovery::Retry(3)).await?)
        })
        .await?;

    let sphere_2_identity = pair_2.client.identity.clone();

    let final_client_version = pair_1
        .spawn(move |mut ctx| async move {
            for value in ["one", "two", "three"] {
                ctx.write(value, &ContentType::Text, value.as_bytes(), None)
                    .await?;
                ctx.save(None).await?;
            }

            ctx.sync(SyncRecovery::Retry(3)).await?;

            ctx.set_petname("two", Some(sphere_2_identity)).await?;

            ctx.save(None).await?;

            ctx.sync(SyncRecovery::Retry(3)).await?;

            for value in ["one", "two", "three"] {
                ctx.set_petname(value, Some(Did(format!("did:key:{}", value))))
                    .await?;
                ctx.save(None).await?;
            }

            ctx.sync(SyncRecovery::Retry(3)).await?;

            wait(1).await;

            Ok(ctx.sync(SyncRecovery::Retry(3)).await?)
        })
        .await?;

    // Stream all of the blocks of client history as represented in gateway's storage
    let block_stream = memo_history_stream(
        pair_1.gateway.workspace.db().await?,
        &final_client_version,
        None,
        true,
    );

    tokio::pin!(block_stream);

    while let Some(_) = block_stream.try_next().await? {}

    ns_task.abort();

    Ok(())
}
