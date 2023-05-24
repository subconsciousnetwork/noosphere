#![cfg(all(feature = "test_kubo", not(target_arch = "wasm32")))]

#[macro_use]
extern crate tracing;

mod helpers;
use anyhow::Result;
use helpers::{start_name_system_server, wait, SpherePair};
use noosphere_core::data::ContentType;
use noosphere_core::tracing::initialize_tracing;
use noosphere_ns::{server::HttpClient, NameResolver};
use noosphere_sphere::{
    HasMutableSphereContext, HasSphereContext, SphereContentRead, SphereContentWrite, SphereCursor,
    SpherePetnameRead, SpherePetnameWrite, SphereReplicaRead, SphereSync, SyncRecovery,
};
use std::sync::Arc;
use tokio::io::AsyncReadExt;
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
    use noosphere_sphere::SphereContentRead;
    use tokio::io::AsyncReadExt;
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

    base_pair.start_gateway().await?;
    other_pair.start_gateway().await?;

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
            let resolved = ctx.resolve_petname("thirdparty").await?;
            assert!(resolved.is_none());
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

    pair_1
        .spawn(|mut ctx| async move {
            ctx.sync(SyncRecovery::Retry(3)).await?;
            wait(1).await;
            ctx.sync(SyncRecovery::Retry(3)).await?;

            let cursor = SphereCursor::latest(Arc::new(ctx.sphere_context().await?.clone()));
            let pair_2_context = cursor
                .traverse_by_petnames(&vec!["pair_2".into()])
                .await?
                .unwrap();

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
