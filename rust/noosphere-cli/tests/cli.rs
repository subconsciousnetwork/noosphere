#![cfg(not(target_arch = "wasm32"))]

mod helpers;

use anyhow::Result;
use helpers::CliSimulator;
use noosphere_cli::paths::SPHERE_DIRECTORY;
use noosphere_core::tracing::initialize_tracing;
use serde_json::Value;

use crate::helpers::wait;

#[tokio::test(flavor = "multi_thread")]
async fn orb_status_errors_on_empty_directory() -> Result<()> {
    initialize_tracing(None);
    let client = CliSimulator::new()?;

    assert!(client.orb(&["sphere", "status"]).await.is_err());

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn orb_sphere_create_initializes_a_sphere() -> Result<()> {
    initialize_tracing(None);
    let client = CliSimulator::new()?;

    client.orb(&["key", "create", "foobar"]).await?;
    client
        .orb(&["sphere", "create", "--owner-key", "foobar"])
        .await?;

    assert!(client.sphere_directory().join(SPHERE_DIRECTORY).exists());

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn orb_can_enable_multiple_replicas_to_synchronize() -> Result<()> {
    initialize_tracing(None);

    let first_replica = CliSimulator::new()?;
    let second_replica = CliSimulator::new()?;

    first_replica.orb(&["key", "create", "foo"]).await?;
    first_replica
        .orb(&["sphere", "create", "--owner-key", "foo"])
        .await?;

    let client_sphere_id = first_replica
        .orb_with_output(&["sphere", "status", "--id"])
        .await?
        .join("\n");

    let gateway = CliSimulator::new()?;

    gateway.orb(&["key", "create", "gateway"]).await?;

    gateway
        .orb(&["sphere", "create", "--owner-key", "gateway"])
        .await?;

    gateway
        .orb(&["sphere", "config", "set", "counterpart", &client_sphere_id])
        .await?;

    let gateway_task = tokio::task::spawn(async move { gateway.orb(&["serve"]).await });

    wait(1).await;

    first_replica
        .orb(&[
            "sphere",
            "config",
            "set",
            "gateway-url",
            "http://127.0.0.1:4433",
        ])
        .await?;

    second_replica.orb(&["key", "create", "bar"]).await?;
    let second_replica_id = match serde_json::from_str(
        &second_replica
            .orb_with_output(&["key", "list", "--as-json"])
            .await?
            .join("\n"),
    )? {
        Value::Object(keys) => keys.get("bar").unwrap().as_str().unwrap().to_owned(),
        _ => panic!(),
    };

    tokio::fs::write(
        first_replica.sphere_directory().join("foo.subtext"),
        "foobar",
    )
    .await?;

    first_replica.orb(&["sphere", "save"]).await?;

    first_replica
        .orb(&["sphere", "auth", "add", &second_replica_id])
        .await?;

    let second_replica_auth = match serde_json::from_str(
        &first_replica
            .orb_with_output(&["sphere", "auth", "list", "--as-json"])
            .await?
            .join("\n"),
    )? {
        Value::Array(auths) => match auths
            .iter()
            .filter(|auth| {
                auth.as_object()
                    .unwrap()
                    .get("name")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    != "(OWNER)"
            })
            .take(1)
            .next()
            .unwrap()
        {
            Value::Object(auth) => auth.get("link").unwrap().as_str().unwrap().to_owned(),
            _ => panic!(),
        },
        _ => panic!(),
    };

    first_replica.orb(&["sphere", "sync"]).await?;

    second_replica
        .orb(&[
            "sphere",
            "join",
            "--authorization",
            &second_replica_auth,
            "--local-key",
            "bar",
            "--gateway-url",
            "http://127.0.0.1:4433",
            &client_sphere_id,
        ])
        .await?;

    let foo_contents =
        tokio::fs::read_to_string(second_replica.sphere_directory().join("foo.subtext")).await?;

    assert_eq!(foo_contents.as_str(), "foobar");

    gateway_task.abort();

    Ok(())
}

#[cfg(feature = "test_kubo")]
mod multiplayer {
    use crate::helpers::{start_name_system_server, wait, CliSimulator, SpherePair};

    use anyhow::Result;
    use noosphere_core::{data::Did, tracing::initialize_tracing};
    use noosphere_sphere::{
        HasMutableSphereContext, SphereAuthorityWrite, SphereContentWrite, SpherePetnameWrite,
        SphereSync, SyncRecovery,
    };
    use serde_json::Value;
    use url::Url;

    #[tokio::test(flavor = "multi_thread")]
    async fn orb_can_render_peers_in_the_sphere_address_book() -> Result<()> {
        initialize_tracing(None);

        let ipfs_url = Url::parse("http://127.0.0.1:5001").unwrap();
        let (ns_url, ns_task) = start_name_system_server(&ipfs_url).await.unwrap();

        let mut pair_1 = SpherePair::new("ONE", &ipfs_url, &ns_url).await?;
        let mut pair_2 = SpherePair::new("TWO", &ipfs_url, &ns_url).await?;
        let mut pair_3 = SpherePair::new("THREE", &ipfs_url, &ns_url).await?;
        let mut pair_4 = SpherePair::new("FOUR", &ipfs_url, &ns_url).await?;
        let mut pair_5 = SpherePair::new("FIVE", &ipfs_url, &ns_url).await?;

        pair_1.start_gateway().await?;
        pair_2.start_gateway().await?;
        pair_3.start_gateway().await?;
        pair_4.start_gateway().await?;
        pair_5.start_gateway().await?;

        let sphere_1_id = pair_1.client.identity.clone();
        let sphere_2_id = pair_2.client.identity.clone();
        let sphere_3_id = pair_3.client.identity.clone();
        let sphere_4_id = pair_4.client.identity.clone();
        let sphere_5_id = pair_5.client.identity.clone();

        for (index, pair) in [&pair_1, &pair_2, &pair_3, &pair_4, &pair_5]
            .iter()
            .enumerate()
        {
            pair.spawn(move |mut ctx| async move {
                let id = index + 1;
                ctx.write(
                    format!("content{}", id).as_str(),
                    "text/plain",
                    format!("foo{}", id).as_bytes(),
                    None,
                )
                .await?;

                ctx.save(None).await?;
                ctx.sync(SyncRecovery::Retry(3)).await?;

                Ok(())
            })
            .await?;
        }

        {
            let sphere_3_id = sphere_3_id.clone();
            let sphere_4_id = sphere_4_id.clone();
            pair_2
                .spawn(move |mut ctx| async move {
                    ctx.set_petname("peer3-of-peer2", Some(sphere_3_id)).await?;
                    ctx.set_petname("peer4", Some(sphere_4_id)).await?;
                    ctx.save(None).await?;
                    ctx.sync(SyncRecovery::Retry(3)).await?;
                    wait(1).await;
                    ctx.sync(SyncRecovery::Retry(3)).await?;
                    Ok(())
                })
                .await?;
        }

        {
            let sphere_2_id = sphere_2_id.clone();
            let sphere_3_id = sphere_3_id.clone();
            pair_1
                .spawn(move |mut ctx| async move {
                    ctx.set_petname("peer2", Some(sphere_2_id)).await?;
                    ctx.set_petname("peer3", Some(sphere_3_id)).await?;
                    ctx.save(None).await?;
                    ctx.sync(SyncRecovery::Retry(3)).await?;
                    wait(1).await;
                    ctx.sync(SyncRecovery::Retry(3)).await?;
                    Ok(())
                })
                .await?;
        }

        let gateway_url = pair_1.client.workspace.gateway_url().await?;
        let cli = CliSimulator::new()?;

        cli.orb(&["key", "create", "foobar"]).await?;

        let cli_id = match serde_json::from_str(
            &cli.orb_with_output(&["key", "list", "--as-json"])
                .await?
                .join("\n"),
        )? {
            Value::Object(keys) => keys.get("foobar").unwrap().as_str().unwrap().to_owned(),
            _ => panic!(),
        };

        let (authorization, sphere_1_version) = pair_1
            .spawn(move |mut ctx| async move {
                let authorization = ctx.authorize("cli", &Did(cli_id)).await?;
                ctx.save(None).await?;
                let version = ctx.sync(SyncRecovery::Retry(3)).await?;
                wait(1).await;
                Ok((authorization, version))
            })
            .await?;

        // Join the first sphere
        cli.orb(&[
            "sphere",
            "join",
            "--authorization",
            &authorization.to_string(),
            "--local-key",
            "foobar",
            "--gateway-url",
            &gateway_url.to_string(),
            &sphere_1_id,
        ])
        .await?;

        let expected_content = [
            ("content1.txt", "foo1"),
            ("@peer2/content2.txt", "foo2"),
            ("@peer3/content3.txt", "foo3"),
            ("@peer2/@peer3-of-peer2/content3.txt", "foo3"),
            ("@peer2/@peer4/content4.txt", "foo4"),
            (".sphere/identity", &sphere_1_id),
            (".sphere/version", &sphere_1_version.to_string()),
        ];

        for (path, content) in expected_content {
            let path = cli.sphere_directory().join(path);

            assert!(tokio::fs::try_exists(&path).await?);
            assert_eq!(&tokio::fs::read_to_string(&path).await?, content);
        }

        // Change a peer-of-my-peer
        pair_4
            .spawn(move |mut ctx| async move {
                ctx.write(
                    "content4",
                    "text/plain",
                    "foo4 and something new".as_bytes(),
                    None,
                )
                .await?;
                ctx.set_petname("peer5", Some(sphere_5_id)).await?;
                ctx.save(None).await?;
                ctx.sync(SyncRecovery::Retry(3)).await?;
                wait(1).await;
                ctx.sync(SyncRecovery::Retry(3)).await?;
                ctx.sync(SyncRecovery::Retry(3)).await?;

                Ok(())
            })
            .await?;

        // Add another level of depth to the graph
        pair_3
            .spawn(move |mut ctx| async move {
                ctx.set_petname("peer4-of-peer3", Some(sphere_4_id)).await?;
                ctx.save(None).await?;
                ctx.sync(SyncRecovery::Retry(3)).await?;
                wait(1).await;
                ctx.sync(SyncRecovery::Retry(3)).await?;

                Ok(())
            })
            .await?;

        // Change a peer
        pair_2
            .spawn(move |mut ctx| async move {
                ctx.write("newcontent", "text/plain", "new".as_bytes(), None)
                    .await?;
                ctx.set_petname("peer4", None).await?;
                ctx.save(None).await?;
                ctx.sync(SyncRecovery::Retry(3)).await?;
                wait(1).await;
                ctx.sync(SyncRecovery::Retry(3)).await?;

                Ok(())
            })
            .await?;

        // Rename a peer
        let sphere_1_version = pair_1
            .spawn(move |mut ctx| async move {
                // Give the graph state the opportunity to "settle"
                wait(2).await;
                ctx.set_petname("peer3", None).await?;
                ctx.set_petname("peer2", None).await?;
                ctx.set_petname("peer2-renamed", Some(sphere_2_id)).await?;
                ctx.save(None).await?;
                ctx.sync(SyncRecovery::Retry(3)).await?;
                wait(2).await;
                let version = ctx.sync(SyncRecovery::Retry(3)).await?;

                Ok(version)
            })
            .await?;

        wait(1).await;

        // Sync to get the latest remote changes
        cli.orb(&["sphere", "sync", "--auto-retry", "3"]).await?;

        let expected_content = [
            ("content1.txt", "foo1"),
            ("@peer2-renamed/content2.txt", "foo2"),
            ("@peer2-renamed/newcontent.txt", "new"),
            ("@peer2-renamed/@peer3-of-peer2/content3.txt", "foo3"),
            (
                "@peer2-renamed/@peer3-of-peer2/@peer4-of-peer3/content4.txt",
                "foo4 and something new",
            ),
            ("@peer2-renamed/@peer3-of-peer2/content3.txt", "foo3"),
            (
                "@peer2-renamed/@peer3-of-peer2/@peer4-of-peer3/content4.txt",
                "foo4 and something new",
            ),
            (".sphere/identity", &sphere_1_id),
            (".sphere/version", &sphere_1_version.to_string()),
        ];

        for (path, content) in expected_content {
            let path = cli.sphere_directory().join(path);

            assert!(
                tokio::fs::try_exists(&path).await?,
                "'{}' should exist",
                path.display()
            );
            assert_eq!(
                &tokio::fs::read_to_string(&path).await?,
                content,
                "'{}' should contain '{content}'",
                path.display()
            );
        }

        let peer_5_content_path =
            "@peer2-renamed/@peer3-of-peer2/@peer4-of-peer3/@peer5/content5.txt";

        let unexpected_content = [
            // Peer removed
            "@peer3/content3.txt",
            // Peer renamed
            "@peer2/content2.txt",
            // Peer removed
            "@peer2-renamed/@peer4/content4.txt",
            // Peer depth greater than render depth
            peer_5_content_path,
        ];

        for path in unexpected_content {
            assert!(
                !tokio::fs::try_exists(&cli.sphere_directory().join(path)).await?,
                "'{path}' should not exist"
            );
        }

        wait(1).await;

        // Sync again, but with a greater render depth
        cli.orb(&["sphere", "sync", "--auto-retry", "3", "--render-depth", "4"])
            .await?;

        // Previously omitted peer should be rendered now
        assert!(
            tokio::fs::try_exists(&cli.sphere_directory().join(peer_5_content_path)).await?,
            "'{peer_5_content_path}' should exist"
        );

        // Re-render using the original render depth
        cli.orb(&["sphere", "render", "--render-depth", "3"])
            .await?;

        assert!(
            !tokio::fs::try_exists(&cli.sphere_directory().join(peer_5_content_path)).await?,
            "'{peer_5_content_path}' should not exist"
        );

        ns_task.abort();

        Ok(())
    }
}
